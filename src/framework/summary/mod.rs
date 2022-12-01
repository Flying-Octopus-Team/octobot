use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

use anyhow::Result;
use chrono::NaiveDate;
use diesel::pg::Pg;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::{http::CacheHttp, model::prelude::MessageId};
use uuid::Uuid;

use self::db_summary::Summary as DbSummary;
use super::meeting::Meeting;
use crate::database::schema::summary::BoxedQuery;
use crate::discord::split_message;
use crate::framework::member::Member;
use crate::framework::report::Report;
use crate::SETTINGS;

pub(super) mod db_summary;

#[derive(Debug, Clone)]
pub struct Summary {
    pub id: Uuid,
    pub note: String,
    pub create_date: NaiveDate,
    pub messages_id: Vec<MessageId>,
}

impl Summary {
    pub(super) fn new(create_date: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4(),
            note: String::new(),
            create_date,
            messages_id: Vec::new(),
        }
    }

    pub(super) fn insert(&self) -> Result<()> {
        let db_summary = DbSummary::from(self.clone());

        db_summary.insert()?;
        Ok(())
    }

    pub(super) fn update(&self) -> Result<()> {
        let db_summary = DbSummary::from(self.clone());
        db_summary.update()?;
        Ok(())
    }

    async fn list(
        filter: Filter,
        cache_http: &impl CacheHttp,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Summary>, i64)> {
        let (db_summary, total) = DbSummary::list(filter, page, page_size)?;

        let mut summaries = Vec::new();

        for db_summary in db_summary {
            let summary = Self::from_db_summary(cache_http, db_summary).await.unwrap();
            summaries.push(summary);
        }

        Ok((summaries, total))
    }

    async fn from_db_summary(cache_http: impl CacheHttp, db_summary: DbSummary) -> Result<Self> {
        let id = db_summary.id();
        let note = String::from(db_summary.note());
        let create_date = db_summary.create_date();
        let messages_id = db_summary.messages_id.unwrap_or_default();

        let mut messages = Vec::new();

        for message_id in messages_id {
            let message_id = message_id.parse::<u64>().unwrap();
            let message = match cache_http.cache() {
                Some(cache) => match cache.message(SETTINGS.summary_channel, message_id) {
                    Some(message) => message,
                    None => {
                        cache_http
                            .http()
                            .get_message(SETTINGS.summary_channel.0, message_id)
                            .await?
                    }
                },
                None => {
                    cache_http
                        .http()
                        .get_message(SETTINGS.summary_channel.0, message_id)
                        .await?
                }
            };
            messages.push(message.id);
        }

        Ok(Self {
            id,
            note,
            create_date,
            messages_id: messages,
        })
    }

    pub async fn get(cache_http: &impl CacheHttp, id: Uuid) -> Result<Self> {
        let db_summary = DbSummary::find_by_id(id)?;
        Self::from_db_summary(cache_http, db_summary).await
    }

    async fn members(&self, cache_http: &impl CacheHttp) -> Result<Vec<Member>> {
        let meeting = Meeting::get_by_summary_id(cache_http, self.id).await?;

        let mut members = Vec::with_capacity(meeting.members.len());

        for (_, member) in meeting.members {
            members.push(member);
        }

        Ok(members)
    }

    async fn generate_summary(
        &self,
        cache_http: &impl CacheHttp,
        note: Option<String>,
        reports: &Vec<Report>,
    ) -> Result<String> {
        let mut summary = String::new();

        let note = note.unwrap_or_else(|| self.note.clone());

        let date_format = "%d.%m.%Y";
        write!(
            summary,
            "**Raport ze spotkania {}**\n\n",
            self.create_date.format(date_format)
        )?;

        summary.push_str("**Na spotkaniu pojawili się:** ");
        let members = self.members(cache_http).await?;

        for member in &members {
            summary.push_str(&member.name());
            if member != members.last().unwrap() {
                summary.push_str(", ");
            }
        }

        summary.push_str("\n\n**Raporty z tego tygodnia:**\n");
        summary.push_str(&self.generate_report_summary(reports).await?);

        summary.push_str("\n**Notatka ze spotkania:**\n");
        summary.push_str(&note);

        Ok(summary)
    }

    async fn generate_report_summary(&self, reports: &Vec<Report>) -> Result<String> {
        let mut summary = String::new();

        let mut last_member_id = Uuid::nil();
        for report in reports {
            if last_member_id != report.member.id {
                summary.push_str(&format!("**{}:**", report.member.name()));
            }
            write!(summary, " {}", report.content)?;
            last_member_id = report.member.id;
        }

        Ok(summary)
    }

    async fn get_reports(
        &self,
        cache_http: &impl CacheHttp,
        add_unpublished: bool,
    ) -> Result<Vec<Report>> {
        let mut reports = Report::get_by_summary_id(cache_http, self.id).await?;

        if add_unpublished {
            let unpublished = Report::get_unpublished(cache_http).await?;
            reports.extend(unpublished);
        }

        reports.sort_by(|a, b| a.member.cmp(&b.member));

        Ok(reports)
    }

    pub(super) async fn send_summary(&mut self, cache_http: &impl CacheHttp) -> Result<String> {
        let reports = self.get_reports(cache_http, true).await?;

        let summary = self.generate_summary(cache_http, None, &reports).await?;

        let messages = split_message(summary)?;

        let channel_id = SETTINGS.summary_channel;

        for message in messages {
            let message = channel_id.say(cache_http.http(), message).await?;

            self.messages_id.push(message.id);
        }

        for mut report in reports {
            report.published = true;
            report.update()?;
        }

        self.update()?;
        Ok("Summary was generated and sent to the channel".to_string())
    }

    pub async fn resend_summary(&self, cache_http: &impl CacheHttp) -> Result<String> {
        let reports = self.get_reports(cache_http, false).await?;

        let summary = self.generate_summary(cache_http, None, &reports).await?;

        let messages = split_message(summary)?;

        if self.messages_id.len() != messages.len() {
            return Err(anyhow::anyhow!("Number of messages is different"));
        }

        for (message, message_id) in messages.iter().zip(self.messages_id.iter()) {
            let channel_id = SETTINGS.summary_channel;

            channel_id
                .edit_message(cache_http.http(), *message_id, |m| m.content(message))
                .await?;
        }

        Ok("Summary was generated and sent to the channel".to_string())
    }

    pub async fn preview_summary(
        &self,
        cache_http: &impl CacheHttp,
        note: Option<String>,
    ) -> Result<String> {
        let reports = self.get_reports(cache_http, true).await?;

        let summary = self.generate_summary(cache_http, note, &reports).await?;

        Ok(summary)
    }

    pub fn find() -> Filter {
        Filter::default()
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Summary ({}): created on {} with {} raports",
            self.id.as_simple(),
            self.create_date,
            self.messages_id.len()
        )
    }
}

pub struct Filter {
    id: Option<Uuid>,
    note: Option<String>,
    create_date: Option<NaiveDate>,
    messages_id: Option<Vec<MessageId>>,
}

impl Filter {
    pub fn new() -> Self {
        Filter {
            id: None,
            note: None,
            create_date: None,
            messages_id: None,
        }
    }

    fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    fn note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }

    fn create_date(mut self, create_date: NaiveDate) -> Self {
        self.create_date = Some(create_date);
        self
    }

    fn messages_id(mut self, messages_id: Vec<MessageId>) -> Self {
        self.messages_id = Some(messages_id);
        self
    }

    pub async fn list(
        self,
        cache_http: &impl CacheHttp,
        page: i64,
        page_size: Option<i64>,
    ) -> Result<(Vec<Summary>, i64)> {
        Summary::list(self, cache_http, page, page_size).await
    }

    pub fn apply(self, mut query: BoxedQuery<'_, Pg>) -> BoxedQuery<'_, Pg> {
        use crate::database::schema::summary::dsl;

        if let Some(id) = self.id {
            query = query.filter(dsl::id.eq(id));
        }

        if let Some(note) = self.note {
            query = query.filter(dsl::note.eq(note));
        }

        if let Some(create_date) = self.create_date {
            query = query.filter(dsl::create_date.eq(create_date));
        }

        if let Some(messages_id) = &self.messages_id {
            let messages_id = messages_id
                .iter()
                .map(|id| id.0.to_string())
                .collect::<Vec<String>>();
            query = query.filter(dsl::messages_id.eq(messages_id));
        }

        query
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<&CommandDataOption> for Filter {
    type Error = Box<dyn std::error::Error>;

    fn try_from(option: &CommandDataOption) -> Result<Self, Self::Error> {
        let mut builder = Filter::new();

        for option in option.options.iter() {
            builder = match option.name.as_str() {
                "id" => {
                    let id = option
                        .value
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .parse::<Uuid>()?;
                    builder.id(id)
                }
                "note" => {
                    let note = option.value.as_ref().unwrap().as_str().unwrap().to_string();
                    builder.note(note)
                }
                "create_date" => {
                    let create_date = option
                        .value
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .parse::<NaiveDate>()?;
                    builder.create_date(create_date)
                }
                "messages_id" => {
                    let messages_id = option
                        .value
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .split(',')
                        .map(|id| id.parse::<u64>().unwrap().into())
                        .collect::<Vec<MessageId>>();
                    builder.messages_id(messages_id)
                }
                _ => builder,
            }
        }

        Ok(builder)
    }
}
