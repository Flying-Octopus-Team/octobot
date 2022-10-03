use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use std::fmt::Write;
use tracing::info;
use uuid::Uuid;

use crate::database::models::summary::Summary;
use crate::database::models::{member::Member, report::Report};
use crate::discord::find_option_as_string;

use super::find_option_value;

pub(crate) async fn add_report(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding report");
    let member_dc_id = match find_option_value(&option.options[..], "member") {
        Some(member_id) => String::from(member_id.as_str().unwrap()),
        None => command
            .member
            .as_ref()
            .unwrap()
            .user
            .id
            .as_u64()
            .to_string(),
    };

    let member = match Member::find_by_discord_id(member_dc_id) {
        Ok(member) => member,
        Err(why) => return Ok(format!("Member not found in the database: {}", why)),
    };

    let content = match find_option_value(&option.options[..], "content") {
        Some(content) => String::from(content.as_str().unwrap()),
        None => return Ok("No content specified".to_string()),
    };

    let summary = match find_option_as_string(&option.options[..], "summary") {
        Some(meeting) => Some(uuid::Uuid::parse_str(&meeting)?),
        None => None,
    };

    let mut report = Report::insert(member.id(), content)?;

    if let Some(summary) = summary {
        let summary = Summary::find_by_id(summary)?;

        report.set_summary_id(summary.id())?;

        if summary.is_published() {
            report.set_publish()?;
        }

        summary.send_summary(ctx, true).await?;
    }

    info!("Report added: {:?}", report);

    Ok(format!("Report added: {}", report))
}

pub(crate) fn remove_report(
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Removing report");
    let report_id = match find_option_value(&option.options[..], "id") {
        Some(report_id) => Uuid::parse_str(report_id.as_str().unwrap()),
        None => return Ok("No report specified".to_string()),
    }?;

    let report = match Report::find_by_id(report_id) {
        Ok(report) => report,
        Err(why) => return Ok(format!("Can't find report with this ID: {why}")),
    };

    match report.delete() {
        Ok(deleted) => match deleted {
            true => {
                info!("Report removed: {:?}", report);
                Ok("Report deleted".to_string())
            }
            false => {
                info!("Removed 0 reports");
                Ok("Deleted 0 reports".to_string())
            }
        },
        Err(err) => Err(err),
    }
}

pub(crate) fn list_reports(
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing reports");
    let page = match find_option_value(&option.options[..], "page") {
        Some(page) => page.as_i64().unwrap(),
        None => 1,
    };

    let page_size =
        find_option_value(&option.options[..], "page-size").map(|v| v.as_i64().unwrap());

    let member = find_option_value(&option.options[..], "member").map(|member_id| {
        let member_dc_id = member_id.as_str().unwrap();
        Member::find_by_discord_id(member_dc_id)
            .map(|member| member.id())
            .unwrap() /*Some(member.id())*/
    });

    let published = find_option_value(&option.options[..], "published")
        .map(|published| published.as_bool().unwrap());

    let (reports, total_pages) = Report::list(page, page_size, member, published)?;

    let mut output = String::new();

    for report in reports {
        writeln!(&mut output, "{}", report)?;
    }

    write!(&mut output, "Page {} of {}", page, total_pages)?;

    Ok(output)
}

pub(crate) fn update_report(
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Updating report");
    let mut old_report = match find_option_value(&option.options[..], "id") {
        Some(report_id) => {
            let report_id = Uuid::parse_str(report_id.as_str().unwrap())?;
            if let Ok(report) = Report::find_by_id(report_id) {
                report
            } else {
                return Ok("Can't find report with this ID".to_string());
            }
        }
        None => return Ok("No report specified".to_string()),
    };

    if let Some(content) = find_option_value(&option.options[..], "content") {
        old_report.content = String::from(content.as_str().unwrap());
    }

    if let Some(member) = find_option_value(&option.options[..], "member") {
        let member_dc_id = member.as_str().unwrap();
        let member = Member::find_by_discord_id(member_dc_id)?;
        old_report.member_id = member.id();
    }

    let report = old_report.update()?;

    info!("Report updated: {:?}", report);

    Ok(format!("Report updated: {}", report))
}
