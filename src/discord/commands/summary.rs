use serenity::prelude::Context;
use std::fmt::Write;
use tracing::info;

use crate::{
    database::models::summary::Summary, discord::find_option_value, meeting::MeetingStatus,
};

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

pub(crate) fn list_summaries(
    option: &serenity::model::prelude::interaction::application_command::CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing summaries");

    let page =
        find_option_value(&option.options[..], "page").map_or(1, |page| page.as_i64().unwrap());

    let page_size = find_option_value(&option.options[..], "page-size")
        .map(|page_size| page_size.as_i64().unwrap());

    let (summaries, total_pages) = Summary::list(page, page_size)?;

    let mut output = String::new();

    for summary in summaries {
        writeln!(&mut output, "{}", summary)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    Ok(output)
}
