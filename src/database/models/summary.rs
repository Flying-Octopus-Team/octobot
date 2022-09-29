use crate::database::schema::summary;
use crate::database::PG_POOL;
use crate::SETTINGS;

use crate::database::pagination::Paginate;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::discord::split_message;
use crate::meeting::MeetingStatus;
use chrono::NaiveDate;
use diesel::Table;
use serenity::prelude::Context;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;
use tracing::info;
use uuid::Uuid;

use super::report::Report;

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

    pub fn insert(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::insert_into(summary::table)
            .values(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn update(&self) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(diesel::update(self)
            .set(self)
            .get_result(&mut PG_POOL.get()?)?)
    }

    pub fn delete(&self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::database::schema::summary::dsl::*;

        Ok(diesel::delete(summary.filter(id.eq(self.id)))
            .execute(&mut PG_POOL.get()?)
            .map(|rows| rows != 0)?)
    }

    pub fn list(
        page: i64,
        per_page: Option<i64>,
    ) -> Result<(Vec<Self>, i64), Box<dyn std::error::Error>> {
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

    pub(crate) fn find_by_id(summary_id: Uuid) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::database::schema::summary::dsl::*;

        Ok(summary
            .filter(id.eq(summary_id))
            .first(&mut PG_POOL.get()?)?)
    }

    /// Set content. Returns the updated summary.
    pub(crate) fn set_note(&mut self, note: String) -> Result<Self, Box<dyn std::error::Error>> {
        self.note = note;

        self.update()
    }

    /// Generate summary for the meeting. Return the summary of reports and the list of members that were present.
    pub(crate) async fn generate_summary(
        &self,
        meeting_status: &MeetingStatus,
        note: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut summary = String::new();

        let date_format = "%d.%m.%Y";
        write!(
            summary,
            "**Raport ze spotkania {}**\n\n",
            meeting_status.start_date().format(date_format)
        )?;

        summary.push_str("**Na spotkaniu pojawili się:** ");
        for member in &meeting_status.members() {
            summary.push_str(&member.member_name());
            // print comma if not last element
            if member != meeting_status.members().last().unwrap() {
                summary.push_str(", ");
            }
        }

        summary.push_str("\n\n**Raporty z tego tygodnia:**\n");
        let save_summary = Summary::find_by_id(meeting_status.summary_id())?;
        summary.push_str(&Report::report_summary(Some(save_summary)).await?);

        summary.push_str("\n**Notatka ze spotkania:**\n");
        summary.push_str(&note);
        Ok(summary)
    }

    /// Generates the summary of the meeting and sends it to the summary channel.
    /// If set to resend. It will resend the summary to the summary channel.
    /// If there are no previous summaries messages to resend to or new summary is too long, it will return an error.
    pub(crate) async fn send_summary(
        meeting_status: &mut MeetingStatus,
        ctx: &Context,
        note: &str,
        resend: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let summary = meeting_status.generate_summary(note.to_string()).await?;

        if summary.is_empty() {
            info!("Generated empty summary");
            return Ok("Summary is empty. Nothing was sent".to_string());
        }
        let messages = split_message(summary)?;

        let channel_id = SETTINGS.summary_channel;

        if resend {
            // edit old messages only if there are the same number of messages
            if let Some(messages_id) = meeting_status.summary_messages_id() {
                if messages_id.len() == messages.len() {
                    for (message_id, message) in messages_id.iter().zip(messages.iter()) {
                        channel_id
                            .edit_message(&ctx.http, message_id.parse::<u64>().unwrap(), |m| {
                                m.content(message)
                            })
                            .await
                            .map_err(|e| format!("Error editing summary: {}", e))?;
                    }
                } else {
                    // if there are different number of messages, return message notifying about it
                    return Err(
                            "New summary is too long to fit in the old messages. Summary was not edited"
                                .into(),
                        );
                }
            } else {
                return Err("No previous summary messages to resend to".into());
            }
        } else {
            let mut messages_id = Vec::new();
            for message in messages {
                let message_id = channel_id
                    .say(&ctx.http, message)
                    .await
                    .map_err(|e| format!("Error sending summary: {}", e))?
                    .id
                    .0;
                messages_id.push(message_id.to_string());
            }

            match meeting_status.set_summary_messages_id(messages_id) {
                Ok(_) => {}
                Err(e) => return Err(format!("Error saving summary: {}", e).into()),
            }
        }

        Ok("Summary was generated and sent to the channel".to_string())
    }

    pub(crate) fn messages_id(&self) -> Option<Vec<String>> {
        self.messages_id.clone()
    }

    pub(crate) fn set_messages_id(
        &self,
        messages_id: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
}

impl Display for Summary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Summary {{ id: {}, create_date: {}, messages_id: {:?} }}",
            self.id, self.create_date, self.messages_id
        )
    }
}
