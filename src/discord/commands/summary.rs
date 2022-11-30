use std::fmt::Write;

use anyhow::Result;
use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use tracing::info;
use tracing::log::error;

use crate::discord::find_option_as_string;
use crate::discord::find_option_value;
use crate::framework::meeting::Meeting;
use crate::framework::summary::Summary;

pub(crate) async fn preview_summary(ctx: Context, option: &CommandDataOption) -> Result<String> {
    info!("Generating summary preview");

    let note = find_option_as_string(&option.options[..], "note");

    let summary = if let Some(summary_id) = find_option_as_string(&option.options[..], "id") {
        let id = uuid::Uuid::parse_str(&summary_id)?;
        Summary::get(&ctx, id).await?
    } else {
        let meeting = Meeting::get_current_meeting(&ctx).await;

        meeting.summary
    };

    let summary = summary.preview_summary(&ctx, note).await?;

    if summary.is_empty() {
        info!("Generated empty summary");
        Ok("Summary is empty".to_string())
    } else {
        Ok(summary)
    }
}

pub(crate) async fn list_summaries(ctx: Context, option: &CommandDataOption) -> Result<String> {
    info!("Listing summaries");

    let page =
        find_option_value(&option.options[..], "page").map_or(1, |page| page.as_i64().unwrap());

    let page_size = find_option_value(&option.options[..], "page-size")
        .map(|page_size| page_size.as_i64().unwrap());

    let (summaries, total_pages) = Summary::find().list(&ctx, page, page_size).await?;

    let mut output = String::new();

    for summary in summaries {
        writeln!(&mut output, "{}", summary)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    Ok(output)
}

pub(crate) async fn resend_summary(ctx: Context, option: &CommandDataOption) -> Result<String> {
    info!("Resending summary");

    let id = match find_option_as_string(&option.options[..], "id") {
        Some(id) => match uuid::Uuid::parse_str(&id) {
            Ok(id) => id,
            Err(why) => {
                let error_msg = format!("Wrong ID value. Could not parse the value: {}", why);
                error!("{}", error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }
        },
        None => {
            let error_msg = "Didn't find ID option in the command".to_string();
            error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }
    };

    let summary = match Summary::get(&ctx, id).await {
        Ok(summary) => summary,
        Err(why) => {
            let error_msg = format!("There's no summary with such ID: {id}. Error: {why}");
            error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }
    };

    let output = summary.resend_summary(&ctx).await?;

    Ok(output)
}
