use crate::database::pagination::Paginate;
use crate::database::schema::report;
use crate::diesel::ExpressionMethods;
use chrono::NaiveDate;
use diesel::{QueryDsl, RunQueryDsl};
use uuid::Uuid;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = report)]
struct Report {
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

    pub fn insert(member_uuid: Uuid, content: String) -> Report {
        let new_report = NewReport {
            member_uuid,
            content,
        };

        diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(&mut crate::database::PG_POOL.get().unwrap())
            .expect("Error creating new report")
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(&self)
            .set(self)
            .get_result(&mut crate::database::PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> bool {
        use crate::database::schema::report::dsl::*;

        diesel::delete(report.filter(id.eq(id)))
            .execute(&mut crate::database::PG_POOL.get().unwrap())
            .is_ok()
    pub fn list(
        page: i64,
        per_page: Option<i64>,
        member_dc_id: Option<Uuid>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        use crate::database::schema::report;
        use crate::database::schema::report::dsl::*;

        let mut query = report::table.into_boxed();

        if let Some(member_dc_id) = member_dc_id {
            query = query.filter(member_uuid.eq(member_dc_id));
        }

        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        let (reports, total_pages) =
            query.load_and_count_pages(&mut crate::database::PG_POOL.get().unwrap())?;
        Ok((reports, total_pages))
    }
    }
}
