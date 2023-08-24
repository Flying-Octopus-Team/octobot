use std::fmt::{Display, Formatter};
use std::str::FromStr;

use chrono::NaiveDateTime;
use cron::Schedule;
use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use tracing::{error, warn};
use uuid::Uuid;

use crate::database::models::member::Member;
use crate::database::models::summary::Summary;
use crate::database::pagination::Paginate;
use crate::database::schema::{meeting, meeting_members};
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::diesel::Table;
use crate::discord::Error;
use crate::SETTINGS;

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

    /// Removes the meeting from the database.
    /// Returns the number of rows affected.
    pub fn delete(&self) -> Result<usize, Error> {
        use crate::database::schema::meeting::dsl::*;

        let summary = Summary::find_by_id(self.summary_id)?;

        let rows = diesel::delete(meeting.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?;

        summary.delete()?;

        Ok(rows)
    }

    /// Saves current time as meeting's end date. And saves itself in the database
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

    pub fn set_schedule(&mut self, new_schedule: String) -> Result<Self, Error> {
        let schedule = Schedule::from_str(&new_schedule)?;
        let next = schedule.upcoming(chrono::Local).next().unwrap();
        self.start_date = next.naive_local();
        self.scheduled_cron = new_schedule;

        match self.update() {
            Ok(s) => Ok(s),
            Err(e) => {
                let error = format!("Error while updating meeting's schedule: {}", e);
                warn!("{}", error);
                Err(anyhow!(error))
            }
        }
    }

    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(&mut self, new_channel_id: String) -> Result<Self, Error> {
        self.channel_id = new_channel_id;

        match self.update() {
            Ok(s) => Ok(s),
            Err(e) => {
                let error = format!("Error while updating meeting's channel id: {}", e);
                warn!("{}", error);
                Err(anyhow!(error))
            }
        }
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

    pub fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(meeting::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn scheduled_cron(&self) -> &str {
        self.scheduled_cron.as_ref()
    }

    /// Loads next meeting based on the previous meeting's cron.
    /// Previous meetings are loaded from the database.
    /// If there is no previous meeting, loads the next meeting based on the default cron.
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
    pub(crate) fn remove_member(&self, member: &Member) -> Result<String, Error> {
        let member_dc_id = member.discord_id().unwrap();
        let mut output = String::new();
        if !match MeetingMembers::is_user_in_meeting(self.id(), member.id()) {
            Ok(is_in_meeting) => is_in_meeting,
            Err(why) => {
                let error_msg = format!(
                    "Error checking if user is in meeting: {}\nReason: {}",
                    self.id(),
                    why
                );
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        } {
            let error_msg = format!("Member <@{}> is not in meeting {}", member_dc_id, self.id());
            warn!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
        match self._remove_member(member.id()) {
            Ok(_) => {
                output.push_str("Removed member <@");
                output.push_str(member_dc_id);
                output.push('>');
            }
            Err(why) => {
                let error_msg = format!(
                    "Error removing member <@{}> from meeting: {}\nReason: {}",
                    member_dc_id,
                    self.id(),
                    why
                );
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        }
        Ok(output)
    }

    fn _remove_member(&self, user_id: Uuid) -> Result<usize, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        let rows = diesel::delete(meeting_members.filter(member_id.eq(user_id)))
            .execute(&mut PG_POOL.get()?)?;
        Ok(rows)
    }

    /// Adds member from the database and from the meeting.
    /// Returns the formatted string with the result.
    pub(crate) fn add_member(&self, member: &Member) -> Result<String, Error> {
        let member_dc_id = member.discord_id().unwrap();
        let mut output = String::new();
        if match MeetingMembers::is_user_in_meeting(self.id(), member.id()) {
            Ok(is_in_meeting) => is_in_meeting,
            Err(why) => {
                let error_msg = format!(
                    "Error checking if user is in meeting: {}\nReason: {}",
                    self.id(),
                    why
                );
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        } {
            let error_msg = format!(
                "Member <@{}> is already in meeting {}",
                member_dc_id,
                self.id()
            );
            warn!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
        match self._add_member(member.id()) {
            Ok(_) => {
                output.push_str("Added member <@");
                output.push_str(&member_dc_id.to_string());
                output.push('>');
            }
            Err(why) => {
                let error_msg = format!(
                    "Error adding member <@{}> to meeting: {}\nReason: {}",
                    member_dc_id,
                    self.id(),
                    why
                );
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        }
        Ok(output)
    }

    fn _add_member(&self, user_id: Uuid) -> Result<MeetingMembers, Error> {
        let meeting_member = MeetingMembers {
            id: Uuid::new_v4(),
            member_id: user_id,
            meeting_id: self.id,
        };

        match meeting_member.insert() {
            Ok(_) => Ok(meeting_member),
            Err(e) => {
                let error = format!("Error while adding member to meeting: {}", e);
                error!("{}", error);
                Err(anyhow!(error))
            }
        }
    }

    /// Check if meeting exists in the database.
    pub(crate) fn exists(&self) -> Result<bool, Error> {
        use crate::database::schema::meeting::dsl::*;

        let count: i64 = meeting
            .filter(id.eq(self.id))
            .count()
            .get_result(&mut PG_POOL.get()?)?;

        Ok(count > 0)
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
            .into_boxed()
            .paginate(page);

        if let Some(page_size) = page_size {
            query = query.per_page(page_size);
        }

        let result = query.load_and_count_pages::<Self>(&mut PG_POOL.get().unwrap())?;

        Ok(result)
    }

    pub(crate) fn members(&self) -> Result<Vec<Member>, Error> {
        let members = MeetingMembers::load_members(self.id)
            .unwrap()
            .into_iter()
            .map(|m| Member::find_by_id(m.member_id))
            .collect::<Result<Vec<Member>, Error>>()?;

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
            .expect("Cannot find user in the database")
            .to_string())
    }

    pub(crate) fn member_id(&self) -> Uuid {
        self.member_id
    }

    /// Checks if a member is already in the meeting
    pub(crate) fn is_user_in_meeting(meeting: Uuid, user_id: Uuid) -> Result<bool, Error> {
        use crate::database::schema::meeting_members::dsl::*;

        let count: i64 = meeting_members
            .filter(meeting_id.eq(meeting))
            .filter(member_id.eq(user_id))
            .count()
            .get_result(&mut PG_POOL.get()?)?;

        Ok(count > 0)
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
        _ctx: &serenity::Context,
        _interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let id = match value {
            serenity::json::Value::String(id) => match Uuid::parse_str(id) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Failed to parse meeting id: {}", id);
                    error!("{}", error_msg);
                    return Err(poise::SlashArgError::Parse {
                        error: Box::new(why),
                        input: id.to_string(),
                    });
                }
            },
            _ => {
                return Err(poise::SlashArgError::CommandStructureMismatch(
                    "Meeting id must be a string",
                ))
            }
        };

        let meeting = match Meeting::find_by_id(id) {
            Ok(meeting) => meeting,
            Err(why) => {
                let error_msg = format!("Failed to get meeting: {}", why);
                error!("{}", error_msg);
                return Err(poise::SlashArgError::Parse {
                    error: why.into(),
                    input: id.to_string(),
                });
            }
        };

        Ok(meeting)
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(serenity::command::CommandOptionType::String);
    }
}
