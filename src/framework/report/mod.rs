use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use anyhow::Result;
use chrono::Local;
use chrono::NaiveDate;
use diesel::pg::Pg;
use serenity::http::CacheHttp;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use self::db_report::Report as DbReport;
use super::member::Member;
use super::summary::Summary;
use crate::database::schema::report::BoxedQuery;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;

mod db_report;

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
    pub fn new(member: Member, content: String) -> Report {
        Report {
            id: Uuid::new_v4(),
            member,
            content,
            create_date: Local::now().date_naive(),
            published: false,
            summary: None,
        }
    }

    pub fn insert(&self) -> Result<()> {
        let db_report = DbReport::from(self.clone());
        db_report.insert()?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
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

    pub fn delete(&mut self) -> Result<bool> {
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

    pub async fn list(
        filter: impl Into<Filter>,
        cache_http: &impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64)> {
        let (db_reports, total_pages) = DbReport::list(filter, page, per_page)?;

        let mut reports = Vec::new();
        for db_report in db_reports {
            let report = Self::from_db_report(cache_http, db_report).await?;

            reports.push(report);
        }

        Ok((reports, total_pages))
    }

    pub async fn get(cache_http: &impl CacheHttp, id: Uuid) -> Result<Report> {
        let db_report = match DbReport::find_by_id(id) {
            Ok(report) => report,
            Err(e) => return Err(e),
        };

        let report = Self::from_db_report(cache_http, db_report).await?;

        Ok(report)
    }

    async fn from_db_report(ctx: &impl CacheHttp, db_report: DbReport) -> Result<Report> {
        let member = Member::get(db_report.member_id, ctx).await?;
        let summary = match db_report.summary_id {
            Some(summary_id) => Some(Summary::get(ctx, summary_id).await?),
            None => None,
        };

        let report = Report {
            id: db_report.id,
            member,
            content: db_report.content,
            create_date: db_report.create_date,
            published: db_report.published,
            summary,
        };

        Ok(report)
    }

    pub async fn get_by_summary_id(cache_http: &impl CacheHttp, id: Uuid) -> Result<Vec<Report>> {
        let db_reports = DbReport::get_by_summary_id(id)?;
        let mut reports = Vec::new();

        for db_report in db_reports {
            reports.push(Report::from_db_report(cache_http, db_report).await?);
        }

        Ok(reports)
    }

    pub async fn get_unpublished(cache_http: &impl CacheHttp) -> Result<Vec<Report>> {
        let db_reports = DbReport::get_unpublished_reports()?;
        let mut reports = Vec::new();

        for db_report in db_reports {
            reports.push(Report::from_db_report(cache_http, db_report).await?);
        }

        Ok(reports)
    }

    pub fn find() -> Filter {
        Filter::default()
    }

    pub fn set_summary(&mut self, summary: Summary) {
        self.summary = Some(summary);
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
    }

    pub fn set_member(&mut self, member: Member) {
        self.member = member;
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

#[derive(Default, Debug, Clone)]
pub struct Filter {
    member_id: Option<Uuid>,
    content: Option<String>,
    create_date: Option<NaiveDate>,
    published: Option<bool>,
    summary_id: Option<Uuid>,
}

impl Filter {
    pub fn apply(self, query: BoxedQuery<'_, Pg>) -> BoxedQuery<'_, Pg> {
        use crate::database::schema::report::dsl;

        let mut query = query;

        if let Some(member_id) = self.member_id {
            query = query.filter(dsl::member_id.eq(member_id));
        }

        if let Some(content) = self.content {
            query = query.filter(dsl::content.eq(content));
        }

        if let Some(create_date) = self.create_date {
            query = query.filter(dsl::create_date.eq(create_date));
        }

        if let Some(published) = self.published {
            query = query.filter(dsl::published.eq(published));
        }

        if let Some(summary_id) = self.summary_id {
            query = query.filter(dsl::summary_id.eq(summary_id));
        }

        query
    }

    pub async fn list(
        self,
        cache_http: &impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Report>, i64)> {
        Report::list(self, cache_http, page, per_page).await
    }

    pub fn member_id(mut self, member: Option<Uuid>) -> Self {
        self.member_id = member;
        self
    }

    pub fn published(mut self, published: Option<bool>) -> Self {
        self.published = published;
        self
    }

    pub fn summary_id(mut self, summary_id: Option<Uuid>) -> Self {
        self.summary_id = summary_id;
        self
    }

    pub fn content(mut self, content: Option<String>) -> Self {
        self.content = content;
        self
    }

    pub fn create_date(mut self, create_date: Option<NaiveDate>) -> Self {
        self.create_date = create_date;
        self
    }
}
