use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use chrono::NaiveDate;
use diesel::pg::Pg;
use serenity::http::CacheHttp;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::database::models::report::Report as DbReport;
use crate::database::models::report::ReportFilter;
use crate::database::schema::report::BoxedQuery;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;

use super::member::Member;
use super::summary::Summary;

#[derive(Debug, Clone)]
pub struct Report {
    pub id: Uuid,
    pub member: Member,
    pub content: String,
    pub create_date: NaiveDate,
    pub published: bool,
    pub summary: Option<Summary>,
}

impl Report {
    pub fn add_report(
        member: Member,
        content: String,
        create_date: NaiveDate,
        published: bool,
        summary: Option<Summary>,
    ) -> Result<Report, Box<dyn std::error::Error>> {
        let report = Self {
            id: Uuid::new_v4(),
            member,
            content,
            create_date,
            published,
            summary,
        };

        report.insert()?;
        Ok(report)
    }

    pub fn insert(&self) -> Result<(), Box<dyn std::error::Error>> {
        let db_report = DbReport::from(self.clone());
        db_report.insert()?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let db_report = DbReport::from(self.clone());

        match db_report.update() {
            Ok(_) => {
                info!("Report updated successfully.");
                Ok(())
            }
            Err(e) => {
                error!("Failed to update report: {}", e);
                Err(e)
            }
        }
    }

    pub fn delete(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let db_report = DbReport::from(self.clone());

        match db_report.delete() {
            Ok(result) => {
                info!("Report deleted: {}", self.id);
                Ok(result)
            }
            Err(e) => {
                error!("Failed to delete report: {}", e);
                Err(e)
            }
        }
    }

    pub async fn edit(&mut self, builder: ReportBuilder) -> Result<(), Box<dyn std::error::Error>> {
        let report_builder = builder;

        let mut report = report_builder.build().await?;
        report.id = self.id;
        *self = report;

        self.update()?;
        Ok(())
    }

    pub async fn list(
        filter: impl Into<ReportFilter>,
        cache_http: &impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
        let (db_reports, total_pages) = DbReport::list(filter, page, per_page)?;

        let mut reports = Vec::new();
        for db_report in db_reports {
            let report = Self::from_db_report(cache_http, db_report).await?;

            reports.push(report);
        }

        Ok((reports, total_pages))
    }

    pub async fn get(
        cache_http: &impl CacheHttp,
        id: Uuid,
    ) -> Result<Report, Box<dyn std::error::Error>> {
        let db_report = match DbReport::find_by_id(id) {
            Ok(report) => report,
            Err(e) => return Err(e),
        };

        let report = Self::from_db_report(cache_http, db_report).await?;

        Ok(report)
    }

    pub async fn from_db_report(
        ctx: &impl CacheHttp,
        db_report: DbReport,
    ) -> Result<Report, Box<dyn std::error::Error>> {
        let member = Member::get(db_report.member_id, ctx).await?;
        let summary = match db_report.summary_id() {
            Some(summary_id) => Some(Summary::get(ctx, summary_id).await?),
            None => None,
        };

        let report = Report {
            id: db_report.id(),
            member,
            content: db_report.content(),
            create_date: db_report.create_date(),
            published: db_report.published(),
            summary,
        };

        Ok(report)
    }

    pub(crate) async fn get_by_summary_id(
        cache_http: &impl CacheHttp,
        id: Uuid,
    ) -> Result<Vec<Report>, Box<dyn std::error::Error>> {
        let db_reports = DbReport::get_by_summary_id(id)?;
        let mut reports = Vec::new();

        for db_report in db_reports {
            reports.push(Report::from_db_report(cache_http, db_report).await?);
        }

        Ok(reports)
    }

    pub(crate) fn filter() -> ReportFilter {
        ReportFilter::default()
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut content = String::new();
        write!(
            content,
            "Report by {} on {}",
            self.member.name(),
            self.create_date
        )?;
        if self.published {
            write!(content, " (published)")?;
        }
        write!(f, " {}", content)
    }
}

pub struct ReportBuilder {
    pub(crate) member: Option<Member>,
    pub(crate) content: Option<String>,
    pub(crate) create_date: Option<NaiveDate>,
    pub(crate) publish: Option<bool>,
    pub(crate) summary: Option<Summary>,
}

impl ReportBuilder {
    pub fn new() -> Self {
        Self {
            member: None,
            content: None,
            create_date: None,
            publish: None,
            summary: None,
        }
    }

    pub fn member(&mut self, member: Member) -> &mut Self {
        self.member = Some(member);
        self
    }

    pub fn content(&mut self, content: String) -> &mut Self {
        self.content = Some(content);
        self
    }

    pub fn create_date(&mut self, create_date: NaiveDate) -> &mut Self {
        self.create_date = Some(create_date);
        self
    }

    pub fn summary(&mut self, summary_id: Summary) -> &mut Self {
        self.summary = Some(summary_id);
        self
    }

    pub async fn build(&self) -> Result<Report, Box<dyn std::error::Error>> {
        let member = self.member.clone().ok_or("Member is required")?;
        let content = self.content.clone().ok_or("Content is required")?;
        let create_date = self
            .create_date
            .unwrap_or_else(|| chrono::offset::Local::now().date_naive());
        let publish = self.summary.is_some() && self.summary.as_ref().unwrap().is_published();
        let summary = self.summary.clone();

        let mut report = Report::add_report(member, content, create_date, publish, summary)?;

        if let Some(summary) = self.summary.clone() {
            report.summary = Some(summary);
        }

        Ok(report)
    }

    pub fn apply_filter<'a>(&'a self, mut query: BoxedQuery<'a, Pg>) -> BoxedQuery<'a, Pg> {
        use crate::database::schema::report::dsl;

        if let Some(member) = &self.member {
            query = query.filter(dsl::member_id.eq(member.id));
        }

        if let Some(content) = &self.content {
            query = query.filter(dsl::content.eq(content));
        }

        if let Some(create_date) = &self.create_date {
            query = query.filter(dsl::create_date.eq(create_date));
        }

        if let Some(publish) = &self.publish {
            query = query.filter(dsl::published.eq(publish));
        }

        if let Some(summary) = &self.summary {
            query = query.filter(dsl::summary_id.eq(summary.id));
        }

        query
    }
}

impl Default for ReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}
