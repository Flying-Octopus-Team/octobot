use anyhow::Result;
use chrono::NaiveDate;
use diesel::pg::Pg;
use diesel::query_dsl::SaveChangesDsl;
use diesel::{QueryDsl, RunQueryDsl};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::database::models::member::Member;
use crate::database::pagination::Paginate;
use crate::database::pagination::Paginated;
use crate::database::schema::report;
use crate::database::schema::report::dsl;
use crate::database::schema::report::BoxedQuery;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::framework::report::ReportFilter;

type AllColumns = (
    report::id,
    report::member_id,
    report::content,
    report::create_date,
    report::published,
    report::summary_id,
);

const ALL_COLUMNS: AllColumns = (
    report::id,
    report::member_id,
    report::content,
    report::create_date,
    report::published,
    report::summary_id,
);

type All = diesel::dsl::Select<crate::database::schema::report::table, AllColumns>;

#[derive(Associations, Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug)]
#[diesel(belongs_to(Member))]
#[diesel(table_name = report)]
pub struct Report {
    pub id: Uuid,
    pub member_id: Uuid,
    pub content: String,
    create_date: NaiveDate,
    published: bool,
    pub summary_id: Option<Uuid>,
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
            create_date: chrono::Local::now().date_naive(),
            published: false,
            summary_id: None,
        }
    }

    pub fn all() -> All {
        dsl::report.select(ALL_COLUMNS)
    }

    pub fn insert(&self) -> Result<Self> {
        Ok(diesel::insert_into(report::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool> {
        use crate::database::schema::report::dsl::*;

        Ok(diesel::delete(report.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        filter: impl Into<ReportFilter>,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64)> {
        let filter = filter.into();

        let query = filter.apply(Report::all().into_boxed());

        let query = Self::paginate(query, page, per_page);

        let (reports, total_pages) = query.load_and_count_pages(&mut PG_POOL.get().unwrap())?;

        Ok((reports, total_pages))
    }

    pub fn paginate(
        query: BoxedQuery<'_, Pg>,
        page: i64,
        per_page: Option<i64>,
    ) -> Paginated<BoxedQuery<'_, Pg>> {
        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        }

        query
    }

    pub fn get_unpublished_reports() -> Result<Vec<Self>> {
        Ok(dsl::report
            .filter(dsl::published.eq(false))
            .load(&mut PG_POOL.get()?)?)
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self> {
        use crate::database::schema::report::dsl::*;

        let uuid = find_id.into();

        Ok(report.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn get_by_summary_id(find_id: Uuid) -> Result<Vec<Self>> {
        use crate::database::schema::report::dsl::*;

        Ok(report
            .filter(summary_id.eq(find_id))
            .order_by(create_date.asc())
            .load(&mut PG_POOL.get()?)?)
    }

    pub(crate) fn summary_id(&self) -> Option<Uuid> {
        self.summary_id
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn content(&self) -> String {
        self.content.clone()
    }

    pub(crate) fn create_date(&self) -> NaiveDate {
        self.create_date
    }

    pub(crate) fn published(&self) -> bool {
        self.published
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

impl From<crate::framework::report::Report> for Report {
    fn from(report: crate::framework::report::Report) -> Self {
        Self {
            id: report.id,
            member_id: report.member.id,
            create_date: report.create_date,
            content: report.content,
            published: report.published,
            summary_id: report.summary.map(|s| s.id),
        }
    }
}
