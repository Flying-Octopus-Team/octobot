use super::Context;
use super::Error;

mod meeting;
mod member;
mod report;
mod summary;

#[poise::command(
    slash_command,
    category = "Member",
    subcommands(
        "member::add_member",
        "member::remove_member",
        "member::update_member",
        "member::list_members"
    )
)]
pub async fn member(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(
    slash_command,
    category = "Report",
    subcommands(
        "report::add_report",
        "report::remove_report",
        "report::update_report",
        "report::list_reports",
    )
)]
pub async fn report(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(
    slash_command,
    category = "Summary",
    subcommands(
        "summary::preview_summary",
        "summary::resend_summary",
        "summary::list_summaries",
    )
)]
pub async fn summary(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(
    slash_command,
    category = "Meeting",
    subcommands(
        "meeting::status_meeting",
        "meeting::end_meeting",
        "meeting::list_meetings",
        "meeting::plan_meeting",
        "meeting::set_note",
        "meeting::add_member",
        "meeting::remove_member",
    )
)]
pub async fn meeting(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}
