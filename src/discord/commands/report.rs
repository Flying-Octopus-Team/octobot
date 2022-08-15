use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
    },
};
use std::fmt::Write;
use uuid::Uuid;

use crate::database::models::{member::Member, report::Report};

use super::find_option_value;

pub(crate) fn add_report(
    _ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let member_dc_id = match find_option_value(&option.options[..], "member") {
        Some(member_uuid) => String::from(member_uuid.as_str().unwrap()),
        None => command.member.as_ref().unwrap().user.id.as_u64().to_string(),
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

    Ok(format!("Report added: {}", report))
}

pub(crate) fn remove_report(
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let report_id = match find_option_value(&option.options[..], "id") {
        Some(report_id) => Uuid::parse_str(report_id.as_str().unwrap()),
        None => return Ok("No report specified".to_string()),
    }?;

    let report = match Report::find_by_id(report_id) {
        Some(report) => report,
        None => return Ok("Can't find report with this ID".to_string()),
    };

    match report.delete() {
        Ok(deleted) => match deleted {
            true => Ok("Report deleted".to_string()),
            false => Ok("Deleted 0 reports".to_string()),
        },
        Err(err) => Err(err),
    }
}

pub(crate) fn list_reports(
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let page = match find_option_value(&option.options[..], "page") {
        Some(page) => page.as_i64().unwrap(),
        None => 1,
    };

    let page_size =
        find_option_value(&option.options[..], "page_size").map(|v| v.as_i64().unwrap());

    let member = find_option_value(&option.options[..], "member").map(|member_uuid| {
        let member_dc_id = member_uuid.as_str().unwrap();
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
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut old_report = match find_option_value(&option.options[..], "id") {
        Some(report_id) => {
            let report_id = Uuid::parse_str(report_id.as_str().unwrap())?;
            if let Some(report) = Report::find_by_id(report_id) {
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
        old_report.member_uuid = member.id();
    }

    let report = old_report.update()?;

    Ok(format!("Report updated: {}", report))
}
