use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::diesel::RunQueryDsl;
use crate::database::models::member::Member;
use crate::database::schema::{meeting, meeting_members};

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = meeting)]
pub struct Meeting {
    pub id: Uuid,
    start_date: NaiveDateTime,
    end_date: Option<NaiveDateTime>,
    summary_id: Option<Uuid>,
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
    ) -> Meeting {
        Meeting {
            id: Uuid::new_v4(),
            start_date: datetime.naive_local(),
            end_date: None,
            summary_id: None,
            scheduled_cron,
        }
    }

    pub fn end_meeting(&mut self, end_date: chrono::DateTime<chrono::Local>) {
        self.end_date = Some(end_date.naive_local());
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
