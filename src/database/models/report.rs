use crate::database::models::member::Member;
use crate::database::pagination::Paginate;
use crate::database::schema::report;
use crate::database::schema::report::dsl;
use crate::diesel::ExpressionMethods;
use chrono::NaiveDate;
use diesel::{QueryDsl, RunQueryDsl};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Associations, Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(belongs_to(Member))]
#[diesel(table_name = report)]
pub struct Report {
    id: Uuid,
    pub member_id: Uuid,
    pub content: String,
    create_date: NaiveDate,
    published: bool,
    summary_id: Option<Uuid>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = report)]
struct NewReport {
    member_id: Uuid,
    content: String,
}

impl Report {
    pub fn new(member_id: Uuid, content: String) -> Report {
        Report {
            id: Uuid::new_v4(),
            member_id,
            content,
            create_date: chrono::Local::now().naive_local().date(),
            published: false,
            summary_id: None,
        }
    }

    pub fn insert(member_id: Uuid, content: String) -> Result<Self, Box<dyn std::error::Error>> {
        let new_report = NewReport { member_id, content };

        Ok(diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(&self)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        Ok(diesel::delete(report.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
        member_dc_id: Option<Uuid>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let mut query = report::table.into_boxed();

        if let Some(member_dc_id) = member_dc_id {
            query = query.filter(dsl::member_id.eq(member_dc_id));
        }

        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        let (reports, total_pages) =
            query.load_and_count_pages(&mut crate::database::PG_POOL.get().unwrap())?;
        Ok((reports, total_pages))
    }

    pub fn get_unpublished_reports() -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        Ok(dsl::report
            .filter(dsl::published.eq(false))
            .load(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn publish(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(&self)
            .set(dsl::published.eq(true))
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        let uuid = find_id.into();

        Ok(report
            .find(uuid)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "Report {} by {} on {}: {}",
            self.id, self.member_id, self.create_date, self.content
        )
    }
}
