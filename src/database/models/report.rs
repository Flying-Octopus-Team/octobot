use crate::database::models::member::Member;
use crate::database::pagination::Paginate;
use crate::database::schema::report;
use crate::database::schema::report::dsl;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use chrono::NaiveDate;
use diesel::{QueryDsl, RunQueryDsl};
use std::fmt::Write;
use std::fmt::{Display, Formatter};
use tracing::error;
use uuid::Uuid;

use super::summary::Summary;

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
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        Ok(diesel::delete(report.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
        member_dc_id: Option<Uuid>,
        published: Option<bool>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let mut query = report::table.into_boxed();

        if let Some(member_dc_id) = member_dc_id {
            query = query.filter(dsl::member_id.eq(member_dc_id));
        }

        if let Some(published) = published {
            query = query.filter(dsl::published.eq(published));
        }

        let mut query = query.paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        let (reports, total_pages) = query.load_and_count_pages(&mut PG_POOL.get().unwrap())?;
        Ok((reports, total_pages))
    }

    pub fn get_unpublished_reports() -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        Ok(dsl::report
            .filter(dsl::published.eq(false))
            .load(&mut PG_POOL.get()?)?)
    }

    pub fn set_publish(&mut self) -> Result<Self, Box<dyn std::error::Error>> {
        self.published = true;
        match self.update() {
            Ok(report) => Ok(report),
            Err(e) => {
                let error = format!("Error publishing report: {}", e);
                error!("{}", error);
                Err(error.into())
            }
        }
    }

    pub(crate) fn set_summary_id(&mut self, id: Uuid) -> Result<Self, Box<dyn std::error::Error>> {
        self.summary_id = Some(id);
        match self.update() {
            Ok(report) => Ok(report),
            Err(e) => {
                let error = format!("Error setting summary id: {}", e);
                error!("{}", error);
                Err(error.into())
            }
        }
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        let uuid = find_id.into();

        Ok(report.find(uuid).get_result(&mut PG_POOL.get()?)?)
    }

    /// Returns formatted list of reports since last summary.
    ///
    /// If the summary is Some and publish is true, it will set the reports as published and set the summary id.
    pub(crate) async fn report_summary(
        summary: Option<Summary>,
        publish: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut reports = Report::get_unpublished_reports()?;

        // get reports associated with summary
        if let Some(summary) = &summary {
            let summary_reports = Report::get_by_summary_id(summary.id())?;

            for report in summary_reports {
                reports.push(report);
            }

            // delete the same reports
            reports.dedup_by(|a, b| a.id == b.id);
        }

        let mut output = String::new();
        reports.sort_by(|a, b| a.member_id.cmp(&b.member_id));
        let mut previous_report: Option<Report> = None;
        for mut report in reports {
            let member = Member::find_by_id(report.member_id)?;

            // if report is from the same member as the previous report, don't print the member's name

            if previous_report.is_some()
                && previous_report.as_ref().unwrap().member_id == report.member_id
            {
                write!(&mut output, " {}", report.content)?;
            } else {
                if previous_report.is_some() {
                    writeln!(&mut output)?;
                }
                write!(&mut output, "**{}:** {}", member.name(), report.content)?;
            }
            if publish {
                if let Some(summary) = &summary {
                    report.set_publish()?;
                    report.set_summary_id(summary.id())?;
                }
            }

            previous_report = Some(report);
        }
        Ok(output)
    }

    fn get_by_summary_id(find_id: Uuid) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        use crate::database::schema::report::dsl::*;

        Ok(report
            .filter(summary_id.eq(find_id))
            .load(&mut PG_POOL.get()?)?)
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let mut output = String::new();
        write!(
            output,
            "Report {} by {} on {}",
            self.id.as_simple(),
            self.member_id.as_simple(),
            self.create_date
        )?;

        if self.published {
            write!(output, " (published)")?;
        }

        write!(output, ": {}", self.content)?;

        write!(f, "{}", output)
    }
}
