use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use std::fmt::Write;
use tracing::info;
use uuid::Uuid;

use crate::discord::find_option_as_string;
use crate::framework::member::Member;
use crate::framework::report::Report;
use crate::framework::report::ReportBuilder;
use crate::framework::summary::Summary;

use super::find_option_value;

pub(crate) async fn add_report(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding report");
    let member_dc_id = match find_option_value(&option.options[..], "member") {
        Some(member_id) => member_id.as_u64().unwrap(),
        None => *command.member.as_ref().unwrap().user.id.as_u64(),
    };

    let member = match Member::get_by_discord_id(member_dc_id, ctx).await {
        Ok(option) => {
            if let Some(member) = option {
                member
            } else {
                return Err("Member not found in the database".into());
            }
        }
        Err(why) => return Err(why),
    };

    let content = match find_option_value(&option.options[..], "content") {
        Some(content) => String::from(content.as_str().unwrap()),
        None => return Ok("No content specified".to_string()),
    };

    let mut report_builder = ReportBuilder::new();
    report_builder.member(member).content(content);

    if let Some(meeting) = find_option_as_string(&option.options[..], "summary") {
        let id = uuid::Uuid::parse_str(&meeting)?;
        match Summary::get(ctx, id).await {
            Ok(summary) => {
                report_builder.summary(summary);
            }
            Err(why) => return Err(why),
        }
    };

    let report = report_builder.build().await?;

    info!("Report added: {:?}", report);

    Ok(format!("Report added: {}", report))
}

pub(crate) async fn remove_report(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Removing report");
    let report_id = match find_option_value(&option.options[..], "id") {
        Some(report_id) => Uuid::parse_str(report_id.as_str().unwrap()),
        None => return Ok("No report specified".to_string()),
    }?;

    let mut report = match Report::get(ctx, report_id).await {
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

pub(crate) async fn list_reports(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing reports");
    let page = match find_option_value(&option.options[..], "page") {
        Some(page) => page.as_i64().unwrap(),
        None => 1,
    };

    let page_size =
        find_option_value(&option.options[..], "page-size").map(|v| v.as_i64().unwrap());

    let member = match find_option_value(&option.options[..], "member") {
        Some(member_id) => {
            let member_id = Uuid::parse_str(member_id.as_str().unwrap())?;
            match Member::get(member_id, ctx).await {
                Ok(member) => Some(member.id),
                Err(why) => return Err(why),
            }
        }
        None => None,
    };

    let published = find_option_value(&option.options[..], "published")
        .map(|published| published.as_bool().unwrap());

    let mut filter = Report::filter();
    filter.member_id(member).published(published);

    let (reports, total_pages) = Report::list(filter, ctx, page, page_size).await?;

    let mut output = String::new();

    for report in reports {
        writeln!(&mut output, "{}", report)?;
    }

    write!(&mut output, "Page {} of {}", page, total_pages)?;

    Ok(output)
}

pub(crate) async fn update_report(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Updating report");

    let mut update_report = match find_option_value(&option.options[..], "id") {
        Some(report_id) => {
            let report_id = Uuid::parse_str(report_id.as_str().unwrap())?;

            if let Ok(report) = Report::get(ctx, report_id).await {
                report
            } else {
                return Ok("Can't find report with this ID".to_string());
            }
        }
        None => return Ok("No report specified".to_string()),
    };

    let mut report_builder = ReportBuilder::new();

    if let Some(content) = find_option_as_string(&option.options[..], "content") {
        report_builder.content(content);
    }

    if let Some(member) = find_option_value(&option.options[..], "member") {
        let member_dc_id = member.as_u64().unwrap();
        let member = Member::get_by_discord_id(member_dc_id, ctx).await?;

        match member {
            Some(member) => report_builder.member(member),
            None => return Ok("Can't find member with this ID".to_string()),
        };
    }

    update_report.update()?;

    info!("Report updated: {:?}", update_report);

    Ok(format!("Report updated: {}", update_report))
}
