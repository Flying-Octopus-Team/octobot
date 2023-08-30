use std::fmt::Write;

use crate::{database::models::summary::Summary, discord::Context, error::Error};

#[poise::command(slash_command, rename = "preview")]
pub(crate) async fn preview_summary(
    ctx: Context<'_>,
    #[description = "Preview summary by ID"] summary: Option<Summary>,
    #[description = "Note to add to the summary"] note: Option<String>,
) -> Result<(), Error> {
    let note = note.unwrap_or_default();

    let summary = if let Some(summary) = summary {
        summary
    } else {
        let meeting_status = ctx.data().meeting_status.write().await;

        Summary::find_by_id(meeting_status.summary_id())?
    };

    let summary = summary.generate_summary(note, false).await?;

    crate::discord::respond(ctx, summary).await
}

#[poise::command(slash_command, rename = "list")]
pub(crate) async fn list_summaries(
    ctx: Context<'_>,
    #[description = "Page number"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
) -> Result<(), Error> {
    let page = page.unwrap_or(1);

    let (summaries, total_pages) = Summary::list(page, page_size)?;

    let mut output = String::new();

    for summary in summaries {
        writeln!(&mut output, "{}\n", summary)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "resend")]
pub(crate) async fn resend_summary(
    ctx: Context<'_>,
    #[description = "ID of the summary to resend"] summary: Summary,
) -> Result<(), Error> {
    let output = summary.send_summary(ctx, true).await?;

    crate::discord::respond(ctx, output).await
}
