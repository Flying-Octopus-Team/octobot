use std::fmt::{Display, Formatter, Write};

use chrono::NaiveDate;
use diesel::{query_dsl::SaveChangesDsl, Table};
use poise::{serenity_prelude as serenity, SlashArgument};
use tracing::{error, info};
use uuid::Uuid;

use super::{meeting::Meeting, report::Report};
use crate::{
    database::{pagination::Paginate, schema::summary, PG_POOL},
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    discord::{split_message, Context},
    error::Error,
    SETTINGS,
};

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Debug)]
#[diesel(table_name = summary)]
pub(crate) struct Summary {
    id: Uuid,
    note: String,
    create_date: NaiveDate,
    messages_id: Option<Vec<String>>,
}

impl Summary {
    pub fn new(content: String, create_date: NaiveDate) -> Summary {
        Summary {
            id: Uuid::new_v4(),
            note: content,
            create_date,
            messages_id: None,
        }
    }

    pub fn insert(&self) -> Result<Self, Error> {
        Ok(diesel::insert_into(summary::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Error> {
        Ok(self.save_changes(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Error> {
        use crate::database::schema::summary::dsl::*;

        Ok(diesel::delete(summary.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(page: i64, per_page: Option<i64>) -> Result<(Vec<Self>, i64), Error> {
        use crate::database::schema::summary::dsl::*;

        let mut query = summary
            .select(summary::all_columns())
            .order(create_date.desc())
            .into_boxed()
            .paginate(page);

        if let Some(per_page) = per_page {
            query = query.per_page(per_page);
        };

        Ok(query.load_and_count_pages(&mut PG_POOL.get().unwrap())?)
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn find_by_id(summary_id: Uuid) -> Result<Self, Error> {
        use crate::database::schema::summary::dsl::*;

        Ok(summary
            .filter(id.eq(summary_id))
            .first(&mut PG_POOL.get()?)?)
    }

    /// Set content. Returns the updated summary.
    pub(crate) fn set_note(&mut self, note: String) -> Result<Self, Error> {
        self.note = note;

        self.update()
    }

    /// Generate summary for the meeting. Return the summary of reports and the
    /// list of members that were present.
    pub(crate) async fn generate_summary(
        &self,
        mut note: String,
        publish: bool,
    ) -> Result<String, Error> {
        let mut summary = String::new();

        if note.is_empty() {
            note = self.note().to_string();
        }

        let meeting = Meeting::find_by_summary_id(self.id)?;

        let date_format = "%d.%m.%Y";
        write!(
            summary,
            "**Raport ze spotkania {}**\n\n",
            meeting.start_date().format(date_format)
        )?;

        let members = meeting.members()?;

        summary.push_str("**Na spotkaniu pojawili się:** ");
        for member in &members {
            summary.push_str(&member.name());
            // print comma if not last element
            if member != members.last().unwrap() {
                summary.push_str(", ");
            }
        }

        summary.push_str("\n\n**Raporty z tego tygodnia:**\n");
        let save_summary = Summary::find_by_id(meeting.summary_id())?;

        summary.push_str(
            &Report::report_summary(Some(save_summary), publish, self.create_date).await?,
        );

        summary.push_str("\n**Notatka ze spotkania:**\n");
        summary.push_str(&note);

        Ok(summary)
    }

    /// Generates the summary of the meeting and sends it to the summary
    /// channel. If set to resend. It will resend the summary to the summary
    /// channel. If there are no previous summaries messages to resend to or
    /// new summary is too long, it will return an error.
    pub(crate) async fn send_summary(
        mut self,
        ctx: Context<'_>,
        resend: bool,
    ) -> Result<String, Error> {
        let summary = self.generate_summary(self.note().to_string(), true).await?;

        if summary.is_empty() {
            info!("Generated empty summary");
            return Ok("Summary is empty. Nothing was sent".to_string());
        }

        let messages = split_message(summary)?;
        let channel_id = SETTINGS.discord.summary_channel;

        if resend {
            // edit old messages only if there are the same number of messages
            if let Some(messages_id) = self.messages_id() {
                if messages_id.len() == messages.len() {
                    for (message_id, message) in messages_id.iter().zip(messages.iter()) {
                        channel_id
                            .edit_message(ctx, message_id.parse::<u64>().unwrap(), |m| {
                                m.content(message)
                            })
                            .await?;
                    }
                } else {
                    return Err(Error::SummaryTooLong);
                }
            } else {
                return Err(Error::NoSummaryMessages);
            }
        } else {
            let mut messages_id = Vec::new();
            for message in messages {
                let message_id = channel_id.say(ctx, message).await?.id.0;

                messages_id.push(message_id.to_string());
            }

            self.set_messages_id(messages_id)?;
        }

        Ok(format!(
            "Summary was generated and sent to the <#{channel_id}>",
            channel_id = channel_id.0
        ))
    }

    pub(crate) fn messages_id(&self) -> Option<Vec<String>> {
        self.messages_id.clone()
    }

    pub(crate) fn set_messages_id(&mut self, messages_id: Vec<String>) -> Result<Self, Error> {
        self.messages_id = Some(messages_id);

        self.update()
    }

    pub(crate) fn note(&self) -> &str {
        &self.note
    }

    pub(crate) fn is_published(&self) -> bool {
        self.messages_id.is_some()
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Summary ({}): created on {} with {} raports",
            self.id.as_simple(),
            self.create_date,
            self.messages_id.as_ref().map_or(0, |v| v.len())
        )
    }
}

#[async_trait::async_trait]
impl SlashArgument for Summary {
    async fn extract(
        _ctx: &serenity::Context,
        _interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
        value: &serenity::json::Value,
    ) -> Result<Self, poise::SlashArgError> {
        let id = match value {
            serenity::json::Value::String(id) => match Uuid::parse_str(id) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Failed to parse summary id: {}", id);
                    error!("{}", error_msg);
                    return Err(poise::SlashArgError::Parse {
                        error: Box::new(why),
                        input: id.to_string(),
                    });
                }
            },
            _ => {
                return Err(poise::SlashArgError::CommandStructureMismatch(
                    "Summary id must be a string",
                ))
            }
        };

        let summary = match Summary::find_by_id(id) {
            Ok(summary) => summary,
            Err(why) => {
                let error_msg = format!("Failed to get summary: {}", why);
                error!("{}", error_msg);
                return Err(poise::SlashArgError::Parse {
                    error: why.into(),
                    input: id.to_string(),
                });
            }
        };

        Ok(summary)
    }

    fn create(builder: &mut serenity::CreateApplicationCommandOption) {
        builder.kind(serenity::command::CommandOptionType::String);
    }
}
