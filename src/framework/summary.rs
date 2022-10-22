use std::fmt::{Display, Formatter};

use chrono::NaiveDate;
use diesel::pg::Pg;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use serde::Serialize;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::{http::CacheHttp, model::prelude::MessageId};
use uuid::Uuid;

use crate::database::models::summary::Summary as DbSummary;
use crate::database::schema::summary::BoxedQuery;
use crate::database::PG_POOL;
use crate::SETTINGS;

pub(crate) struct Summary {
    id: Uuid,
    note: String,
    create_date: NaiveDate,
    messages_id: Vec<MessageId>,
}

impl Summary {
    pub async fn list(
        filter: SummaryBuilder,
        cache_http: &impl CacheHttp,
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Summary>, i64), Box<dyn std::error::Error>> {
        let query = filter.apply_filter(DbSummary::all().into_boxed());
        let query = DbSummary::paginate(query, page, per_page);

        let (db_summary, total) = query
            .load_and_count_pages(&mut PG_POOL.get().unwrap())
            .unwrap();

        let mut summaries = Vec::new();

        for db_summary in db_summary {
            let summary = Self::from_db_summary(cache_http, db_summary).await.unwrap();
            summaries.push(summary);
        }

        Ok((summaries, total))
    }

    async fn from_db_summary(
        cache_http: &impl CacheHttp,
        db_summary: DbSummary,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

pub struct SummaryBuilder {
    id: Option<Uuid>,
    note: Option<String>,
    create_date: Option<NaiveDate>,
    messages_id: Option<Vec<MessageId>>,
}

impl SummaryBuilder {
    pub fn new() -> Self {
        SummaryBuilder {
            id: None,
            note: None,
            create_date: None,
            messages_id: None,
        }
    }

    pub fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    pub fn note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }

    pub fn create_date(mut self, create_date: NaiveDate) -> Self {
        self.create_date = Some(create_date);
        self
    }

    pub fn messages_id(mut self, messages_id: Vec<MessageId>) -> Self {
        self.messages_id = Some(messages_id);
        self
    }

    pub(crate) fn build(self) -> Summary {
        Summary {
            id: self.id.unwrap(),
            note: self.note.unwrap(),
            create_date: self.create_date.unwrap(),
            messages_id: self.messages_id.unwrap(),
        }
    }

    pub fn apply_filter<'a>(&'a self, mut query: BoxedQuery<'a, Pg>) -> BoxedQuery<'a, Pg> {
        use crate::database::schema::summary::dsl;

        if let Some(id) = &self.id {
            query = query.filter(dsl::id.eq(id));
        }

        if let Some(note) = &self.note {
            query = query.filter(dsl::note.eq(note));
        }

        if let Some(create_date) = &self.create_date {
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

impl TryFrom<&CommandDataOption> for SummaryBuilder {
    type Error = Box<dyn std::error::Error>;

    fn try_from(option: &CommandDataOption) -> Result<Self, Self::Error> {
        let mut builder = SummaryBuilder::new();

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
