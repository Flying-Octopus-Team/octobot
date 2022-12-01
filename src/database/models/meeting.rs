use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

use anyhow::Result;
use chrono::NaiveDateTime;
use cron::Schedule;
use diesel::pg::Pg;
use diesel::query_dsl::SaveChangesDsl;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
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
use crate::framework::meeting::Filter;

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

    pub fn try_from_cron(scheduled_cron: &str, channel_id: String) -> Result<Self> {
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
    pub fn delete(&self) -> Result<usize> {
        use crate::database::schema::meeting::dsl::*;

        let summary = Summary::find_by_id(self.summary_id)?;

        let rows = diesel::delete(meeting.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?;

        summary.delete()?;

        Ok(rows)
    }

    pub(crate) fn list(
        filter: impl Into<Filter>,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Self>, i64)> {
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
    pub fn end_meeting(&mut self, new_end_date: chrono::DateTime<chrono::Local>) -> Result<Self> {
        self.end_date = Some(new_end_date.naive_local());

        self.update()
    }

    pub fn schedule(&self) -> Result<Schedule> {
        Ok(Schedule::from_str(&self.scheduled_cron)?)
    }

    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(&mut self, new_channel_id: String) -> Result<Self> {
        self.channel_id = new_channel_id;

        match self.update() {
            Ok(s) => Ok(s),
            Err(e) => {
                let error = format!("Error while updating meeting's channel id: {}", e);
                warn!("{}", error);
                Err(anyhow::anyhow!(error))
            }
        }
    }

    /// Set summary note
    pub fn set_summary_note(&mut self, note: String) -> Result<()> {
        let mut summary = Summary::find_by_id(self.summary_id)?;
        summary.set_note(note)?;

        Ok(())
    }

    pub fn get_latest_meeting() -> Result<Self> {
        use crate::database::schema::meeting::dsl::*;

        Ok(meeting
            .select(meeting::all_columns())
            .order(start_date.desc())
            .first(&mut PG_POOL.get()?)?)
    }

    pub fn insert(&self) -> Result<Self> {
        Ok(diesel::insert_into(meeting::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn scheduled_cron(&self) -> &str {
        self.scheduled_cron.as_ref()
    }

    pub(crate) fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self> {
        use crate::database::schema::meeting::dsl::*;

        let uuid = find_id.into();

        Ok(meeting.find(uuid).get_result(&mut PG_POOL.get()?)?)
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

    pub(crate) fn members(&self) -> Result<Vec<Member>> {
        let members = MeetingMembers::load_members(self.id)
            .unwrap()
            .into_iter()
            .map(|m| Member::find_by_id(m.member_id))
            .collect::<Result<Vec<Member>>>()?;

        Ok(members)
    }

    pub(crate) fn find_by_summary_id(find_id: Uuid) -> Result<Self> {
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

    pub(crate) fn insert(&self) -> Result<Self> {
        Ok(diesel::insert_into(meeting_members::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn delete_by_meeting_and_member(meeting: Uuid, member: Uuid) -> Result<usize> {
        use crate::database::schema::meeting_members::dsl::*;

        Ok(diesel::delete(
            meeting_members
                .filter(meeting_id.eq(meeting))
                .filter(member_id.eq(member)),
        )
        .execute(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn member_id(&self) -> Uuid {
        self.member_id
    }

    /// Checks if a member is already in the meeting
    pub(crate) fn is_user_in_meeting(meeting: Uuid, user_id: Uuid) -> Result<bool> {
        use crate::database::schema::meeting_members::dsl::*;

        let count: i64 = meeting_members
            .filter(meeting_id.eq(meeting))
            .filter(member_id.eq(user_id))
            .count()
            .get_result(&mut PG_POOL.get()?)?;

        Ok(count > 0)
    }

    pub(crate) fn load_members(find_meeting_id: Uuid) -> Result<Vec<Self>> {
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

impl From<crate::framework::meeting::Meeting> for Meeting {
    fn from(meeting: crate::framework::meeting::Meeting) -> Self {
        Meeting {
            id: meeting.id,
            summary_id: meeting.summary.id,
            start_date: meeting.start_date,
            end_date: meeting.end_date,
            channel_id: meeting.channel.id.0.to_string(),
            scheduled_cron: meeting.schedule.to_string(),
        }
    }
}
