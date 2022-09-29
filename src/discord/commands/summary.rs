use serenity::{
    model::prelude::interaction::application_command::CommandDataOption, prelude::Context,
};
use std::fmt::Write;
use tracing::{info, log::error};

use crate::{
    database::models::{meeting::Meeting, summary::Summary},
    discord::{find_option_as_string, find_option_value},
    meeting::MeetingStatus,
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
    option: &CommandDataOption,
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

pub(crate) async fn resend_summary(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Resending summary");

    let id = match find_option_as_string(&option.options[..], "id") {
        Some(id) => match uuid::Uuid::parse_str(&id) {
            Ok(id) => id,
            Err(why) => {
                let error_msg = format!("Wrong ID value. Could not parse the value: {}", why);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        },
        None => {
            let error_msg = "Didn't find ID option in the command".to_string();
            error!("{}", error_msg);
            return Err(error_msg.into());
        }
    };

    let summary = match Summary::find_by_id(id) {
        Ok(summary) => summary,
        Err(why) => {
            let error_msg = format!("There's no summary with such ID: {id}. Error: {why}");
            error!("{}", error_msg);
            return Err(error_msg.into());
        }
    };

    let meeting = Meeting::find_by_summary_id(summary.id())?;

    let output =
        Summary::send_summary(&mut MeetingStatus::from(meeting), ctx, summary.note(), true).await?;

    Ok(output)
}
