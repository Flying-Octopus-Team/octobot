use serenity::prelude::Context;
use tracing::info;

use crate::{database::models::report::Report, SETTINGS};

pub(crate) async fn generate_summary(ctx: &Context) -> Result<String, Box<dyn std::error::Error>> {
    info!("Generating summary");

    let summary = Report::report_summary(None).await?;

    if summary.is_empty() {
        info!("Generated empty summary");
        Ok("Summary is empty. Nothing was sent".to_string())
    } else {
        // send summary to the channel
        let channel_id = SETTINGS.meeting.summary_channel;
        match channel_id
            .send_message(&ctx.http, |m| m.content(summary))
            .await
        {
            Ok(_) => {
                info!("Generated summary and sent it to the channel");
                Ok("Summary was sent".to_string())
            }
            Err(e) => {
                let error = format!("Error sending summary to the channel: {:?}", e);
                info!(?e, "Generated summary but failed to send it to the channel");
                Err(error.into())
            }
        }
    }
}
