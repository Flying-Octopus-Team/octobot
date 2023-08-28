use super::summary::Summary;
use crate::database::models::member::Member;
use crate::database::pagination::Paginate;
use crate::database::schema::report;
use crate::database::schema::report::dsl;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::error::Error;

use chrono::NaiveDate;
use diesel::query_dsl::SaveChangesDsl;
use diesel::{QueryDsl, RunQueryDsl};
use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use std::fmt::Write;
use std::fmt::{Display, Formatter};
use tracing::error;
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

    pub fn insert(member_id: Uuid, content: String) -> Result<Self, Error> {
        let new_report = NewReport { member_id, content };

        Ok(diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<usize, Error> {
        use crate::database::schema::report::dsl::*;

        Ok(diesel::delete(report.filter(id.eq(self.id))).execute(&mut PG_POOL.get()?)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
        member_id: Option<Uuid>,
        published: Option<bool>,
    ) -> Result<(Vec<Self>, i64), Error> {
        let mut query = report::table.into_boxed().order(dsl::create_date.desc());

        if let Some(member_id) = member_id {
            query = query.filter(dsl::member_id.eq(member_id));
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

    /// Returns all unpublished reports from before the given date.
    pub fn get_unpublished_reports(date: NaiveDate) -> Result<Vec<Self>, Error> {
        Ok(dsl::report
            .filter(dsl::published.eq(false))
            .filter(dsl::create_date.lt(date))
            .load(&mut PG_POOL.get()?)?)
    }

    pub fn set_publish(&mut self) -> Result<Self, Error> {
        self.published = true;

        self.update()
    }

    pub(crate) fn summary_id(&self) -> Option<Uuid> {
        self.summary_id
    }

    pub(crate) fn set_summary_id(&mut self, id: Uuid) -> Result<Self, Error> {
        self.summary_id = Some(id);

        self.update()
    }

    pub fn find_by_id(find_id: impl Into<Uuid>) -> Result<Self, Error> {
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
        date: NaiveDate,
    ) -> Result<String, Error> {
        let mut reports = Report::get_unpublished_reports(date)?;

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

    fn get_by_summary_id(find_id: Uuid) -> Result<Vec<Self>, Error> {
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

#[async_trait::async_trait]
impl SlashArgument for Report {
    async fn extract(
        _ctx: &serenity::Context,
        _interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let id = match value {
            serenity::json::Value::String(id) => match Uuid::parse_str(id) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Failed to parse report id: {}", id);
                    error!("{}", error_msg);
                    return Err(poise::SlashArgError::Parse {
                        error: Box::new(why),
                        input: id.to_string(),
                    });
                }
            },
            _ => {
                return Err(poise::SlashArgError::CommandStructureMismatch(
                    "Report id must be a string",
                ))
            }
        };

        let report = match Report::find_by_id(id) {
            Ok(report) => report,
            Err(why) => {
                let error_msg = format!("Failed to get report: {}", why);
                error!("{}", error_msg);
                return Err(poise::SlashArgError::Parse {
                    error: why.into(),
                    input: id.to_string(),
                });
            }
        };

        Ok(report)
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(serenity::command::CommandOptionType::String);
    }
}
