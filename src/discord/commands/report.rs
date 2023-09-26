use std::fmt::Write;

use tracing::info;

use crate::{
    database::models::{member::Member, report::Report, summary::Summary},
    discord::Context,
    error::Error,
};

#[poise::command(slash_command, rename = "add")]
pub(crate) async fn add_report(
    ctx: Context<'_>,
    #[description = "Report's content"] content: String,
    #[description = "Member of the organization"] member: Option<Member>,
    #[description = "Summary's ID"] summary: Option<Summary>,
) -> Result<(), Error> {
    let mut member = match member {
        Some(member) => member,
        None => {
            let author_id = ctx.author().id.to_string();
            Member::find_by_discord_id(author_id)?
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

    member.update_activity(report.create_date)?;

    info!("Report added: {:?}", report);

    let output = format!("Added: {}", report);

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "remove")]
pub(crate) async fn remove_report(
    ctx: Context<'_>,
    #[description = "Report's ID"] report: Report,
) -> Result<(), Error> {
    let mut output = String::new();

    match report.delete() {
        Ok(rows) => {
            if rows == 1 {
                info!("Report removed: {:?}", report);
                writeln!(&mut output, "Removed: {}", report)?;
            } else {
                info!("Removed {rows} reports");
                writeln!(&mut output, "Removed {rows} reports")?;
            }
        }
        Err(err) => return Err(err),
    };

    let mut member = Member::find_by_id(report.member_id)?;

    let last_activity = member.last_activity();

    if let Some(last_activity) = last_activity {
        if report.create_date >= last_activity {
            member.refresh_activity()?;
        }
    }

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "list")]
pub(crate) async fn list_reports(
    ctx: Context<'_>,
    #[description = "Page number"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
    #[description = "Member of the organization"] member: Option<Member>,
    #[description = "Published"] published: Option<bool>,
    #[description = "Summary's ID"] summary: Option<Summary>,
) -> Result<(), Error> {
    let page = page.unwrap_or(1);

    let member = member.map(|member| member.id());

    let (reports, total_pages) = Report::list(page, page_size, member, published, summary)?;

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
    #[description = "Member of the organization"] member: Option<Member>,
    #[description = "Summary's ID"] summary: Option<Summary>,
) -> Result<(), Error> {
    if let Some(content) = content {
        report.content = content;
    }

    let old_member = if let Some(ref member) = member {
        let old_member = Member::find_by_id(report.member_id)?;

        report.member_id = member.id();

        Some(old_member)
    } else {
        None
    };

    if let Some(summary) = summary {
        report.set_summary_id(summary.id())?;
    }

    let report = report.update()?;

    if let Some(mut member) = member {
        member.update_activity(report.create_date)?;

        if let Some(mut old_member) = old_member {
            let old_last_activity = old_member.last_activity();

            if let Some(old_last_activity) = old_last_activity {
                if report.create_date >= old_last_activity {
                    old_member.refresh_activity()?;
                }
            }
        }
    }

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

    output.push_str(&format!("\nUpdated: {}", report));

    crate::discord::respond(ctx, output).await
}
