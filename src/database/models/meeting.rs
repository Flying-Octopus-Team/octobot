use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::database::models::member::Member;
use crate::database::schema::{meeting, meeting_members};
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::diesel::Table;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = meeting)]
pub struct Meeting {
    pub id: Uuid,
    start_date: NaiveDateTime,
    end_date: Option<NaiveDateTime>,
    pub scheduled_cron: String,
    pub summary_id: Uuid,
    channel_id: String,
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

    /// Returns meeting's id.
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn end_meeting(&mut self, end_date: chrono::DateTime<chrono::Local>) {
        self.end_date = Some(end_date.naive_local());
    }


    pub fn channel_id(&self) -> &str {
        self.channel_id.as_ref()
    }

    pub fn set_channel_id(&mut self, channel_id: String) {
        self.channel_id = channel_id;
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
}
