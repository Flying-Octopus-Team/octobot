use serenity::model::application::interaction::application_command::CommandDataOption;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::prelude::Context;
use std::fmt::Write;
use tracing::info;
use uuid::Uuid;

use crate::database::models::{member::Member, report::Report};

use super::find_option_value;

pub(crate) fn add_report(
    _ctx: &Context,
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

    let find_by_discord_id = Member::find_by_discord_id(member_dc_id);
    let member = if let Err(why) = find_by_discord_id {
        return Ok(format!("Member not found: {}", why));
    } else {
        find_by_discord_id?
    };

    let content = match find_option_value(&option.options[..], "content") {
        Some(content) => String::from(content.as_str().unwrap()),
        None => return Ok("No content specified".to_string()),
    };

    let report = Report::insert(member.id(), content)?;

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
        find_option_value(&option.options[..], "page_size").map(|v| v.as_i64().unwrap());

    let member = find_option_value(&option.options[..], "member").map(|member_id| {
        let member_dc_id = member_id.as_str().unwrap();
        Member::find_by_discord_id(member_dc_id)
            .map(|member| member.id())
            .unwrap() /*Some(member.id())*/
    });

    let (reports, total_pages) = Report::list(page, page_size, member)?;

    let mut output = String::new();

    for report in reports {
        writeln!(&mut output, "{}", report)?;
    }

    write!(&mut output, "Page {} of {}", page, total_pages)?;

    Ok(output)
}

pub(crate) fn update_report(
    _ctx: &Context,
    _command: &ApplicationCommandInteraction,
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

pub(crate) async fn summary(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    publish: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Summarizing reports");
    let mut reports = Report::get_unpublished_reports()?;

    let mut output = String::new();

    reports.sort_by(|a, b| a.member_id.cmp(&b.member_id));

    let mut previous_report: Option<Report> = None;

    for report in reports {
        let member = Member::find_by_id(report.member_id)?;
        let member = ctx
            .http
            .get_user(member.discord_id().unwrap().parse()?)
            .await?;

        // if report is from the same member as the previous report, don't print the member's name

        if previous_report.is_some() && previous_report.unwrap().member_id == report.member_id {
            write!(&mut output, " {}", report.content)?;
        } else {
            write!(&mut output, "\n**{}:** {}", member.name, report.content)?;
        }

        if publish {
            report.publish()?;
        }

        previous_report = Some(report);
    }

    if output.is_empty() {
        info!("No reports to summarize");
        Ok("No unpublished reports".to_string())
    } else {
        info!("Summary generated");
        Ok(output)
    }
}
