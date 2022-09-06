use std::str::FromStr;

use chrono::NaiveDateTime;
use cron::Schedule;
use tracing::warn;
use uuid::Uuid;

use crate::database::models::member::Member;
use crate::database::models::summary::Summary;
use crate::database::schema::{meeting, meeting_members};
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::diesel::Table;
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
        let summary = Summary::new("".to_string(), datetime.date().naive_local());

        Meeting {
            id: Uuid::new_v4(),
            start_date: datetime.naive_local(),
            end_date: None,
            summary_id: summary.insert().unwrap().id(),
            channel_id,
            scheduled_cron,
        }
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

        let rows = diesel::delete(meeting.filter(id.eq(self.id)))
            .execute(&mut crate::database::PG_POOL.get()?)?;

        summary.delete()?;

        Ok(rows)
    }

    /// Saves current time as meeting's end date. Struct has to be manually inserted into the database.
    pub fn end_meeting(&mut self, end_date: chrono::DateTime<chrono::Local>) {
        self.end_date = Some(end_date.naive_local());
    }

    pub fn schedule(&self) -> Result<Schedule, Box<dyn std::error::Error>> {
        Ok(Schedule::from_str(&self.scheduled_cron)?)
    }

    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(&mut self, channel_id: String) {
        self.channel_id = channel_id;
    }

    /// Set summary note
    pub fn set_summary_note(&mut self, note: String) -> Result<(), Box<dyn std::error::Error>> {
        let summary = Summary::find_by_id(self.summary_id)?;
        summary.set_note(note)?;
        Ok(())
    }

    pub fn get_latest_meeting() -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .select(meeting::all_columns())
            .order(start_date.desc())
            .first(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(meeting::table)
            .values(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(meeting::table)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
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

        Ok(meeting
            .find(uuid)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub(crate) fn remove_member(&self, user_id: Uuid) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        let rows = diesel::delete(meeting_members.filter(member_id.eq(user_id)))
            .execute(&mut crate::database::PG_POOL.get()?)?;
        Ok(rows)
    }

    pub(crate) fn add_member(
        &self,
        user_id: Uuid,
    ) -> Result<MeetingMembers, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting_members::dsl::*;

        let meeting_member = MeetingMembers {
            id: Uuid::new_v4(),
            member_id: user_id,
            meeting_id: self.id,
        };

        Ok(diesel::insert_into(meeting_members)
            .values(&meeting_member)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    /// Check if meeting exists in the database.
    pub(crate) fn exists(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::meeting::dsl::*;

        let count: i64 = meeting
            .filter(id.eq(self.id))
            .count()
            .get_result(&mut crate::database::PG_POOL.get()?)?;

        Ok(count > 0)
    }

    pub(crate) fn summary_id(&self) -> Uuid {
        self.summary_id
    }

    pub(crate) fn start_date(&self) -> chrono::NaiveDateTime {
        self.start_date
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
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub(crate) fn discord_id(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(Member::find_by_id(self.member_id)?
            .discord_id()
            .expect("Cannot find user in the database")
            .to_string())
    }

    pub(crate) fn _member_id(&self) -> Uuid {
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
            .get_result(&mut crate::database::PG_POOL.get()?)?;

        Ok(count > 0)
    }

    /// Returns member's display name
    pub(crate) fn member_name(&self) -> String {
        let member = Member::find_by_id(self.member_id).unwrap();
        member.name()
    }
}
