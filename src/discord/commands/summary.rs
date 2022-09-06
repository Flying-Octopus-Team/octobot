use serenity::prelude::Context;
use tracing::info;

use crate::meeting::MeetingStatus;

pub(crate) async fn generate_summary(ctx: &Context) -> Result<String, Box<dyn std::error::Error>> {
    info!("Generating summary");

    let read = ctx.data.read().await;
    let meeting_status = read.get::<MeetingStatus>().unwrap().clone();
    let meeting_status = meeting_status.write().await;

    let summary = meeting_status.generate_summary("".to_string()).await?;

    if summary.is_empty() {
        info!("Generated empty summary");
        Ok("Summary is empty. Nothing was sent".to_string())
    } else {
        Ok(summary)
    }
}
