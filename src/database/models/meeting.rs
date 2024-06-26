use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use chrono::NaiveDateTime;
use cron::Schedule;
use diesel::{dsl::exists, query_dsl::SaveChangesDsl, select, BoolExpressionMethods, QueryDsl};
use poise::{
    serenity_prelude::{self as serenity, CommandInteraction, Context, CreateCommandOption},
    SlashArgument,
};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    database::{
        models::{member::Member, summary::Summary},
        pagination::Paginate,
        schema::{meeting, meeting_members},
        PG_POOL,
    },
    diesel::{ExpressionMethods, RunQueryDsl, Table},
    error::Error,
    SETTINGS,
};

#[derive(Default, Queryable, Identifiable, Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = meeting)]
pub struct Meeting {
    pub id: Uuid,
    pub start_date: NaiveDateTime,
    pub end_date: Option<NaiveDateTime>,
    pub summary_id: Uuid,
    channel_id: String,
    scheduled_cron: String,
}

#[derive(Associations, Queryable, Identifiable, Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = meeting_members)]
#[diesel(belongs_to(Meeting))]
#[diesel(belongs_to(Member))]
pub struct MeetingMembers {
    id: Uuid,
    member_id: Uuid,
    meeting_id: Uuid,
}

impl Meeting {
    pub(crate) fn new(
        datetime: chrono::DateTime<chrono::Local>,
        scheduled_cron: String,
        channel_id: String,
    ) -> Meeting {
        let summary = Summary::new(String::new(), datetime.date_naive());

        Meeting {
            id: Uuid::new_v4(),
            start_date: datetime.naive_local(),
            end_date: None,
            summary_id: summary.insert().unwrap().id(),
            channel_id,
            scheduled_cron,
        }
    }

    pub fn try_from_cron(scheduled_cron: &str, channel_id: String) -> Result<Self, Error> {
        let schedule = Schedule::from_str(scheduled_cron)?;
        let next = schedule.upcoming(chrono::Local).next().unwrap();
        Ok(Meeting::new(next, scheduled_cron.to_string(), channel_id))
    }

    /// Returns meeting's id.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Removes the meeting and its summary from the database.
    /// Returns the number of rows affected.
    pub fn delete(&self) -> Result<usize, Error> {
        use crate::database::schema::meeting::dsl::*;

        let summary = Summary::find_by_id(self.summary_id)?;

        let rows = diesel::delete(meeting.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?;

        summary.delete()?;

        Ok(rows)
    }

    /// Saves current time as meeting's end date. And saves itself in the
    /// database
    pub fn end_meeting(
        &mut self,
        new_end_date: chrono::DateTime<chrono::Local>,
    ) -> Result<Self, Error> {
        self.end_date = Some(new_end_date.naive_local());

        self.update()
    }

    pub fn schedule(&self) -> Result<Schedule, Error> {
        Ok(Schedule::from_str(&self.scheduled_cron)?)
    }

    pub fn set_schedule(&mut self, new_schedule: Schedule) -> Result<Self, Error> {
        let next = new_schedule.upcoming(chrono::Local).next().unwrap();
        self.start_date = next.naive_local();
        self.scheduled_cron = new_schedule.to_string();

        self.update()
    }

    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(&mut self, new_channel_id: String) -> Result<Self, Error> {
        self.channel_id = new_channel_id;

        self.update()
    }

    /// Set summary note
    pub fn set_summary_note(&mut self, note: String) -> Result<(), Error> {
        let mut summary = Summary::find_by_id(self.summary_id)?;
        summary.set_note(note)?;

        Ok(())
    }

    pub fn get_latest_meeting() -> Result<Self, Error> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .select(meeting::all_columns())
            .order(start_date.desc())
            .first(&mut PG_POOL.get()?)?)
    }

    pub fn get_previous_meeting() -> Result<Self, Error> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .select(meeting::all_columns())
            .order(start_date.desc())
            .offset(1)
            .first(&mut PG_POOL.get()?)?)
    }

    pub fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(meeting::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn scheduled_cron(&self) -> &str {
        self.scheduled_cron.as_ref()
    }

    /// Loads next meeting based on the previous meeting's cron.
    /// Previous meetings are loaded from the database.
    /// If there is no previous meeting, loads the next meeting based on the
    /// default cron.
    pub fn load_next_meeting() -> Result<Self, Error> {
        let meeting = if let Ok(latest_meeting) = Meeting::get_latest_meeting() {
            if latest_meeting.end_date.is_none() {
                latest_meeting
            } else {
                Meeting::try_from_cron(&latest_meeting.scheduled_cron, latest_meeting.channel_id)?
            }
        } else {
            warn!("No previous meetings found in the database. Falling back to default settings.");
            Meeting::try_from_cron(
                &SETTINGS.meeting.cron,
                SETTINGS.meeting.channel_id.to_string(),
            )?
        };
        Ok(meeting)
    }

    pub(crate) fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Error> {
        use crate::database::schema::meeting::dsl::*;

        let uuid = find_id.into();

        Ok(meeting.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    /// Removes member from the database and from the meeting.
    /// Returns the formatted string with the result.
    pub(crate) fn remove_member(&self, member: &mut Member) -> Result<String, Error> {
        let rows = self._remove_member(member.id())?;

        let mut output = String::new();

        if rows > 0 {
            if let Some(end_date) = self.end_date {
                member.update_activity(end_date.date())?;
            }

            output.push_str("Removed member <@");
            output.push_str(member.discord_id().unwrap());
            output.push('>');
        } else {
            output.push_str("Member <@");
            output.push_str(member.discord_id().unwrap());
            output.push_str("> is not in the meeting");
        }

        Ok(output)
    }

    fn _remove_member(&self, user_id: Uuid) -> Result<usize, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        let rows = diesel::delete(
            meeting_members.filter(member_id.eq(user_id).and(meeting_id.eq(self.id))),
        )
        .execute(&mut PG_POOL.get()?)?;

        Ok(rows)
    }

    /// Adds member from the database and from the meeting.
    /// Returns the formatted string with the result.
    pub(crate) fn add_member(&self, member: &mut Member) -> Result<String, Error> {
        let mut output = String::new();

        if MeetingMembers::is_user_in_meeting(self.id(), member.id())? {
            return Err(Error::UserAlreadyInMeeting {
                user_id: member.id(),
                meeting_id: self.id(),
            })?;
        }

        self._add_member(member.id())?;

        if let Some(end_date) = self.end_date {
            member.update_activity(end_date.date())?;
        }

        output.push_str("Added member <@");
        output.push_str(&member.discord_id().unwrap().to_string());
        output.push('>');

        Ok(output)
    }

    fn _add_member(&self, user_id: Uuid) -> Result<MeetingMembers, Error> {
        MeetingMembers::new(user_id, self.id).insert()
    }

    /// Check if meeting exists in the database.
    pub(crate) fn exists(&self) -> Result<bool, Error> {
        use crate::database::schema::meeting::dsl::*;

        Ok(select(exists(meeting.filter(id.eq(self.id)))).get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn summary_id(&self) -> Uuid {
        self.summary_id
    }

    pub(crate) fn start_date(&self) -> chrono::NaiveDateTime {
        self.start_date
    }

    pub(crate) fn list(page: i64, page_size: Option<i64>) -> Result<(Vec<Self>, i64), Error> {
        use crate::database::schema::meeting::dsl::*;

        let mut query = meeting
            .select(meeting::all_columns())
            .order_by(start_date.desc())
            .into_boxed()
            .paginate(page);

        if let Some(page_size) = page_size {
            query = query.per_page(page_size);
        }

        let result = query.load_and_count_pages::<Self>(&mut PG_POOL.get().unwrap())?;

        Ok(result)
    }

    pub(crate) fn members(&self) -> Result<Vec<Member>, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        let members = meeting_members
            .filter(meeting_id.eq(self.id))
            .inner_join(crate::database::schema::member::table)
            .select(crate::database::schema::member::all_columns)
            .load::<Member>(&mut PG_POOL.get()?)?;

        Ok(members)
    }

    pub(crate) fn find_by_summary_id(find_id: Uuid) -> Result<Self, Error> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .filter(summary_id.eq(find_id))
            .get_result(&mut PG_POOL.get()?)?)
    }
}

impl MeetingMembers {
    pub(crate) fn new(member_id: impl Into<Uuid>, meeting_id: impl Into<Uuid>) -> MeetingMembers {
        MeetingMembers {
            id: Uuid::new_v4(),
            member_id: member_id.into(),
            meeting_id: meeting_id.into(),
        }
    }

    pub(crate) fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(meeting_members::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn discord_id(&self) -> Result<String, Error> {
        Ok(Member::find_by_id(self.member_id)?
            .discord_id()
            .expect("Member should have discord id")
            .to_string())
    }

    pub(crate) fn member_id(&self) -> Uuid {
        self.member_id
    }

    /// Checks if a member is already in the meeting
    pub(crate) fn is_user_in_meeting(meeting: Uuid, user_id: Uuid) -> Result<bool, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        Ok(select(exists(
            meeting_members.filter(meeting_id.eq(meeting).and(member_id.eq(user_id))),
        ))
        .get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn load_members(find_meeting_id: Uuid) -> Result<Vec<Self>, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        let members = meeting_members
            .filter(meeting_id.eq(find_meeting_id))
            .load::<MeetingMembers>(&mut PG_POOL.get()?)?;

        Ok(members)
    }
}

impl PartialEq for MeetingMembers {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Display for Meeting {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Meeting ID: {}\nStart Date: {}\nEnd Date: {:?}\nSummary ID: {}\nMembers: {}",
            self.id.as_simple(),
            self.start_date,
            self.end_date,
            self.summary_id.as_simple(),
            self.members().unwrap().len()
        )
    }
}

#[async_trait::async_trait]
impl SlashArgument for Meeting {
    async fn extract(
        _ctx: &Context,
        _interaction: &CommandInteraction,
        value: &serenity::ResolvedValue<'_>,
    ) -> Result<Self, poise::SlashArgError> {
        let id = match value {
            serenity::ResolvedValue::String(id) => match Uuid::parse_str(id) {
                Ok(id) => id,
                Err(_why) => {
                    let error_msg = format!("Failed to parse meeting id: {}", id);
                    error!("{}", error_msg);
                    // return Err(poise::SlashArgError::Parse {
                    //     error: Box::new(why),
                    //     input: id.to_string(),
                    // });
                    // FIXME: SlashArgError::Parse is marked as non_exhaustive, thus it can't be
                    // constructed.
                    return Err(poise::SlashArgError::new_command_structure_mismatch(
                        "Failed to parse meeting id",
                    ));
                }
            },
            _ => {
                return Err(poise::SlashArgError::new_command_structure_mismatch(
                    "Meeting id must be a string",
                ));
            }
        };

        let meeting = match Meeting::find_by_id(id) {
            Ok(meeting) => meeting,
            Err(why) => {
                let error_msg = format!("Failed to get meeting: {}", why);
                error!("{}", error_msg);
                // return Err(poise::SlashArgError::Parse {
                //     error: why.into(),
                //     input: id.to_string(),
                // });
                // FIXME: SlashArgError::Parse is marked as non_exhaustive, thus it can't be
                // constructed.
                return Err(poise::SlashArgError::new_command_structure_mismatch(
                    "Failed to get meeting",
                ));
            }
        };

        Ok(meeting)
    }

    fn create(builder: CreateCommandOption) -> CreateCommandOption {
        builder.kind(poise::serenity_prelude::CommandOptionType::String)
    }
}
