use crate::database::pagination::Paginate;
use crate::database::schema::report;
use crate::database::schema::report::dsl;
use crate::diesel::ExpressionMethods;
use chrono::NaiveDate;
use diesel::{QueryDsl, RunQueryDsl};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = report)]
pub struct Report {
    id: Uuid,
    member_uuid: Uuid,
    content: String,
    create_date: NaiveDate,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = report)]
struct NewReport {
    member_uuid: Uuid,
    content: String,
}

impl Report {
    pub fn new(member_uuid: Uuid, content: String) -> Report {
        Report {
            id: Uuid::new_v4(),
            member_uuid,
            content,
            create_date: chrono::Local::now().naive_local().date(),
        }
    }

    pub fn insert(member_uuid: Uuid, content: String) -> Result<Self, Box<dyn std::error::Error>> {
        let new_report = NewReport {
            member_uuid,
            content,
        };

        Ok(diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(&mut crate::database::PG_POOL.get().unwrap())?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(&self)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        Ok(diesel::delete(report.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get().unwrap())
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
        member_dc_id: Option<Uuid>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let mut query = report::table.into_boxed();

        if let Some(member_dc_id) = member_dc_id {
            query = query.filter(dsl::member_uuid.eq(member_dc_id));
        }

        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        let (reports, total_pages) =
            query.load_and_count_pages(&mut crate::database::PG_POOL.get().unwrap())?;
        Ok((reports, total_pages))
    }

    pub fn find_by_id(find_id: Uuid) -> Option<Report> {
        use crate::database::schema::report::dsl::*;

        report
            .filter(id.eq(find_id))
            .first(&mut crate::database::PG_POOL.get().unwrap())
            .ok()
    }

    pub fn set_member_uuid(&mut self, member_uuid: Uuid) {
        self.member_uuid = member_uuid;
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "Report {} by {} on {}: {}",
            self.id, self.member_uuid, self.create_date, self.content
        )
    }
}
