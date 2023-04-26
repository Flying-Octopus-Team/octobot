use super::meeting::Meeting;
use super::report::Report;
use crate::database::pagination::Paginate;
use crate::database::schema::summary;
use crate::database::PG_POOL;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::discord::split_message;
use crate::discord::Context;
use crate::discord::Error;
use crate::SETTINGS;

use chrono::NaiveDate;
use diesel::Table;
use poise::serenity_prelude as serenity;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;

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
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
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

    /// Generate summary for the meeting. Return the summary of reports and the list of members that were present.
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

        summary.push_str(&Report::report_summary(Some(save_summary), publish).await?);

        summary.push_str("\n**Notatka ze spotkania:**\n");
        summary.push_str(&note);
        Ok(summary)
    }

    /// Generates the summary of the meeting and sends it to the summary channel.
    /// If set to resend. It will resend the summary to the summary channel.
    /// If there are no previous summaries messages to resend to or new summary is too long, it will return an error.
    pub(crate) async fn send_summary(
        self,
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
                            .await
                            .map_err(|e| anyhow!(format!("Error editing summary: {}", e)))?;
                    }
                } else {
                    // if there are different number of messages, return message notifying about it
                    return Err(anyhow!(
                        "New summary is too long to fit in the old messages. Summary was not edited"
                    ));
                }
            } else {
                return Err(anyhow!("No previous summary messages to resend to"));
            }
        } else {
            let mut messages_id = Vec::new();
            for message in messages {
                let message_id = channel_id
                    .say(ctx, message)
                    .await
                    .map_err(|e| anyhow!(format!("Error sending summary: {}", e)))?
                    .id
                    .0;
                messages_id.push(message_id.to_string());
            }

            match self.set_messages_id(messages_id) {
                Ok(_) => {}
                Err(e) => {
                    error!("Error saving summary: {}", e);
                    return Err(anyhow!("Error saving summary: {}", e));
                }
            }
        }

        Ok("Summary was generated and sent to the channel".to_string())
    }

    pub(crate) fn messages_id(&self) -> Option<Vec<String>> {
        self.messages_id.clone()
    }

    pub(crate) fn set_messages_id(&self, messages_id: Vec<String>) -> Result<Self, Error> {
        let summary = Summary {
            id: self.id,
            note: self.note.clone(),
            create_date: self.create_date,
            messages_id: Some(messages_id),
        };

        summary.update()
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
