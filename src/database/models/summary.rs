use crate::database::schema::summary;

use crate::database::pagination::Paginate;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::meeting::MeetingStatus;
use chrono::NaiveDate;
use diesel::Table;
use std::fmt::Write;
use uuid::Uuid;

use super::report::Report;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = summary)]
pub(crate) struct Summary {
    id: Uuid,
    note: String,
    create_date: NaiveDate,
}

impl Summary {
    pub fn new(content: String, create_date: NaiveDate) -> Summary {
        Summary {
            id: Uuid::new_v4(),
            note: content,
            create_date,
        }
    }

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(summary::table)
            .values(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(&self)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::summary::dsl::*;

        Ok(diesel::delete(summary.filter(id.eq(self.id)))
            .execute(&mut crate::database::PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn _list(
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        use crate::database::schema::summary::dsl::*;

        let mut query = summary
            .select(summary::all_columns())
            .into_boxed()
            .paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        Ok(query.load_and_count_pages(&mut crate::database::PG_POOL.get().unwrap())?)
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn find_by_id(summary_id: Uuid) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::summary::dsl::*;

        Ok(summary
            .filter(id.eq(summary_id))
            .first(&mut crate::database::PG_POOL.get()?)?)
    }

    /// Set content. Returns the updated summary.
    pub(crate) fn set_note(&self, new_content: String) -> Result<Self, Box<dyn std::error::Error>> {
        let summary = Summary {
            id: self.id,
            note: new_content,
            create_date: self.create_date,
        };

        summary.update()
    }

    /// Generate summary for the meeting. Return the summary of reports and the list of members that were present.
    pub(crate) async fn generate_summary(
        &self,
        meeting_status: &MeetingStatus,
        note: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut summary = String::new();

        let date_format = "%d.%m.%Y";
        write!(
            summary,
            "**Raport ze spotkania {}:**",
            meeting_status.start_date().format(date_format)
        )?;

        summary.push_str("**Na spotkaniu pojawili się:**");
        for member in &meeting_status.members() {
            summary.push_str(&member.member_name());
            summary.push_str(", ");
        }

        summary.push_str("**Raporty:**\n");
        let save_summary = Summary::find_by_id(meeting_status.summary_id())?;
        summary.push_str(&Report::report_summary(Some(save_summary)).await?);

        summary.push_str("**Notatka:**\n");
        summary.push_str(&note);
        Ok(summary)
    }
}
