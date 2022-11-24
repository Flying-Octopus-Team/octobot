use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use chrono::NaiveDateTime;
use cron::Schedule;
use diesel::pg::Pg;
use tracing::error;
use tracing::warn;
use uuid::Uuid;

use crate::database::models::member::Member;
use crate::database::models::summary::Summary;
use crate::database::pagination::Paginate;
use crate::database::pagination::Paginated;
use crate::database::schema::meeting;
use crate::database::schema::meeting::BoxedQuery;
use crate::database::schema::meeting_members;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::diesel::Table;
use crate::SETTINGS;
use diesel::query_dsl::SaveChangesDsl;

type AllColumns = (
    meeting::id,
    meeting::start_date,
    meeting::end_date,
    meeting::summary_id,
    meeting::channel_id,
    meeting::scheduled_cron,
);

const ALL_COLUMNS: AllColumns = (
    meeting::id,
    meeting::start_date,
    meeting::end_date,
    meeting::summary_id,
    meeting::channel_id,
    meeting::scheduled_cron,
);

type All = diesel::dsl::Select<meeting::table, AllColumns>;

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
        let summary = Summary::new(String::new(), datetime.date().naive_local());

        Meeting {
            id: Uuid::new_v4(),
            start_date: datetime.naive_local(),
            end_date: None,
            summary_id: summary.insert().unwrap().id(),
            channel_id,
            scheduled_cron,
        }
    }

    pub fn all() -> All {
        meeting::table.select(ALL_COLUMNS)
    }

    pub fn try_from_cron(
        scheduled_cron: &str,
        channel_id: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
    pub fn delete(&self) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting::dsl::*;

        let summary = Summary::find_by_id(self.summary_id)?;

        let rows = diesel::delete(meeting.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?;

        summary.delete()?;

        Ok(rows)
    }

    pub(crate) fn list(
        filter: impl Into<MeetingFilter>,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let filter = filter.into();

        let query = filter.apply(Meeting::all().into_boxed());

        let query = Self::paginate(query, page, page_size);

        let (meetings, total) = query.load_and_count_pages(&mut PG_POOL.get().unwrap())?;

        Ok((meetings, total))
    }

    pub fn paginate(
        query: BoxedQuery<'_, Pg>,
        page: i64,
        page_size: Option<i64>,
    ) -> Paginated<BoxedQuery<'_, Pg>> {
        let mut query = query.paginate(page);

        if let Some(page_size) = page_size {
            query = query.per_page(page_size);
        }

        query
    }

    /// Saves current time as meeting's end date. And saves itself in the database
    pub fn end_meeting(
        &mut self,
        new_end_date: chrono::DateTime<chrono::Local>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        self.end_date = Some(new_end_date.naive_local());

        self.update()
    }

    pub fn schedule(&self) -> Result<Schedule, Box<dyn std::error::Error>> {
        Ok(Schedule::from_str(&self.scheduled_cron)?)
    }

    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(
        &mut self,
        new_channel_id: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        self.channel_id = new_channel_id;

        match self.update() {
            Ok(s) => Ok(s),
            Err(e) => {
                let error = format!("Error while updating meeting's channel id: {}", e);
                warn!("{}", error);
                Err(error.into())
            }
        }
    }

    /// Set summary note
    pub fn set_summary_note(&mut self, note: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut summary = Summary::find_by_id(self.summary_id)?;
        summary.set_note(note)?;

        Ok(())
    }

    pub fn get_latest_meeting() -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .select(meeting::all_columns())
            .order(start_date.desc())
            .first(&mut PG_POOL.get()?)?)
    }

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(meeting::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn scheduled_cron(&self) -> &str {
        self.scheduled_cron.as_ref()
    }

    /// Loads next meeting based on the previous meeting's cron.
    /// Previous meetings are loaded from the database.
    /// If there is no previous meeting, loads the next meeting based on the default cron.
    pub fn load_next_meeting() -> Result<Self, Box<dyn std::error::Error>> {
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

    pub(crate) fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting::dsl::*;

        let uuid = find_id.into();

        Ok(meeting.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    /// Removes member from the database and from the meeting.
    /// Returns the formatted string with the result.
    pub(crate) fn remove_member(
        &self,
        member: &Member,
    ) -> Result<String, Box<dyn std::error::Error>> {
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
                return Err(error_msg.into());
            }
        } {
            let error_msg = format!("Member <@{}> is not in meeting {}", member_dc_id, self.id());
            warn!("{}", error_msg);
            return Err(error_msg.into());
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
                return Err(error_msg.into());
            }
        }
        Ok(output)
    }

    fn _remove_member(&self, user_id: Uuid) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        let rows = diesel::delete(meeting_members.filter(member_id.eq(user_id)))
            .execute(&mut PG_POOL.get()?)?;
        Ok(rows)
    }

    /// Adds member from the database and from the meeting.
    /// Returns the formatted string with the result.
    pub(crate) fn add_member(&self, member: &Member) -> Result<String, Box<dyn std::error::Error>> {
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
                return Err(error_msg.into());
            }
        } {
            let error_msg = format!(
                "Member <@{}> is already in meeting {}",
                member_dc_id,
                self.id()
            );
            warn!("{}", error_msg);
            return Err(error_msg.into());
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
                return Err(error_msg.into());
            }
        }
        Ok(output)
    }

    fn _add_member(&self, user_id: Uuid) -> Result<MeetingMembers, Box<dyn std::error::Error>> {
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
                Err(error.into())
            }
        }
    }

    /// Check if meeting exists in the database.
    pub(crate) fn exists(&self) -> Result<bool, Box<dyn std::error::Error>> {
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

    pub(crate) fn end_date(&self) -> Option<chrono::NaiveDateTime> {
        self.end_date
    }

    pub(crate) fn members(&self) -> Result<Vec<Member>, Box<dyn std::error::Error>> {
        let members = MeetingMembers::load_members(self.id)
            .unwrap()
            .into_iter()
            .map(|m| Member::find_by_id(m.member_id))
            .collect::<Result<Vec<Member>, Box<dyn std::error::Error>>>()?;

        Ok(members)
    }

    pub(crate) fn find_by_summary_id(find_id: Uuid) -> Result<Self, Box<dyn std::error::Error>> {
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

    pub(crate) fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(meeting_members::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn delete(&self) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        Ok(diesel::delete(meeting_members.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn delete_by_meeting_and_member(
        meeting: Uuid,
        member: Uuid,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        Ok(diesel::delete(
            meeting_members
                .filter(meeting_id.eq(meeting))
                .filter(member_id.eq(member)),
        )
        .execute(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn discord_id(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(Member::find_by_id(self.member_id)?
            .discord_id()
            .expect("Cannot find user in the database")
            .to_string())
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn member_id(&self) -> Uuid {
        self.member_id
    }

    /// Checks if a member is already in the meeting
    pub(crate) fn is_user_in_meeting(
        meeting: Uuid,
        user_id: Uuid,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        let count: i64 = meeting_members
            .filter(meeting_id.eq(meeting))
            .filter(member_id.eq(user_id))
            .count()
            .get_result(&mut PG_POOL.get()?)?;

        Ok(count > 0)
    }

    pub(crate) fn load_members(
        find_meeting_id: Uuid,
    ) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
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
            "Meeting ID: {} Summary: {} Start Date: {} End Date: {:?} Members: {}",
            self.id,
            self.summary_id,
            self.start_date,
            self.end_date,
            self.members().unwrap().len()
        )
    }
}

pub struct MeetingFilter {
    start_date: Option<chrono::NaiveDateTime>,
    end_date: Option<chrono::NaiveDateTime>,
    summary_id: Option<Uuid>,
    channel_id: Option<String>,
}

impl MeetingFilter {
    pub fn new() -> MeetingFilter {
        MeetingFilter {
            start_date: None,
            end_date: None,
            summary_id: None,
            channel_id: None,
        }
    }

    fn apply(self, query: BoxedQuery<'_, Pg>) -> BoxedQuery<'_, Pg> {
        let mut query = query;

        if let Some(start_date) = self.start_date {
            query = query.filter(meeting::start_date.gt(start_date));
        }

        if let Some(end_date) = self.end_date {
            query = query.filter(meeting::end_date.lt(end_date));
        }

        if let Some(summary_id) = self.summary_id {
            query = query.filter(meeting::summary_id.eq(summary_id));
        }

        if let Some(channel_id) = self.channel_id {
            query = query.filter(meeting::channel_id.eq(channel_id));
        }

        query
    }

    pub(crate) fn start_date(mut self, start_date: Option<chrono::NaiveDateTime>) -> Self {
        self.start_date = start_date;
        self
    }

    pub(crate) fn end_date(mut self, end_date: Option<chrono::NaiveDateTime>) -> Self {
        self.end_date = end_date;
        self
    }

    pub(crate) fn summary_id(mut self, summary_id: Option<Uuid>) -> Self {
        self.summary_id = summary_id;
        self
    }

    pub(crate) fn channel_id(mut self, channel_id: Option<String>) -> Self {
        self.channel_id = channel_id;
        self
    }
}
