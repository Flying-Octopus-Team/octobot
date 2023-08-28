use std::fmt::Write;

use tracing::info;

use super::Error;
use crate::database::models::summary::Summary;
use crate::database::models::{member::Member, report::Report};
use crate::discord::Context;

#[poise::command(slash_command, rename = "add")]
pub(crate) async fn add_report(
    ctx: Context<'_>,
    #[description = "Report's content"] content: String,
    #[description = "Member's ID"] member: Option<Member>,
    #[description = "Summary's ID"] summary: Option<Summary>,
) -> Result<(), Error> {
    info!("Adding report");
    let member = match member {
        Some(member) => member,
        None => {
            let author_id = ctx.author().id.to_string();
            match Member::find_by_discord_id(author_id) {
                Ok(member) => member,
                Err(why) => return Err(anyhow!("Member not found in the database: {}", why)),
            }
        }
    };

    let mut report = Report::insert(member.id(), content)?;

    if let Some(summary) = summary {
        report.set_summary_id(summary.id())?;

        if summary.is_published() {
            report.set_publish()?;
        }

        summary.send_summary(ctx, true).await?;
    }

    info!("Report added: {:?}", report);

    let output = format!("Report added: {}", report);

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "remove")]
pub(crate) async fn remove_report(
    ctx: Context<'_>,
    #[description = "Report's ID"] report: Report,
) -> Result<(), Error> {
    info!("Removing report");

    let mut output = String::new();

    match report.delete() {
        Ok(deleted) => match deleted {
            true => {
                info!("Report removed: {:?}", report);
                writeln!(&mut output, "Report removed: {}", report)?;
            }
            false => {
                info!("Removed 0 reports");
                writeln!(&mut output, "Removed 0 reports")?;
            }
        },
        Err(err) => return Err(err),
    };

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "list")]
pub(crate) async fn list_reports(
    ctx: Context<'_>,
    #[description = "Page number"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
    #[description = "Member's ID"] member: Option<Member>,
    #[description = "Published"] published: Option<bool>,
) -> Result<(), Error> {
    info!("Listing reports");
    let page = page.unwrap_or(1);

    let member = member.map(|member| member.id());

    let (reports, total_pages) = Report::list(page, page_size, member, published)?;

    let mut output = String::new();

    for report in reports {
        writeln!(&mut output, "{}\n", report)?;
    }

    write!(&mut output, "Page {} of {}", page, total_pages)?;

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "update")]
pub(crate) async fn update_report(
    ctx: Context<'_>,
    #[description = "Report's ID"] mut report: Report,
    #[description = "Report's content"] content: Option<String>,
    #[description = "Member's ID"] member: Option<Member>,
) -> Result<(), Error> {
    info!("Updating report");

    if let Some(content) = content {
        report.content = content;
    }

    if let Some(member) = member {
        report.member_id = member.id();
    }

    let report = match report.update() {
        Ok(report) => report,
        Err(why) => return Err(anyhow!("Can't update report: {}", why)),
    };

    let mut output = String::new();

    if let Some(summary_id) = report.summary_id() {
        info!("Updating summary");
        let summary = Summary::find_by_id(summary_id)?;

        match summary.send_summary(ctx, true).await {
            Ok(_) => writeln!(&mut output, "Summary updated")?,
            Err(why) => writeln!(&mut output, "Can't update summary: {}", why)?,
        }
    }

    info!("Report updated: {:?}", report);

    output.push_str(&format!("\nReport updated: {}", report));

    crate::discord::respond(ctx, output).await
}
