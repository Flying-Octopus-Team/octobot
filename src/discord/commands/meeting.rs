use std::fmt::Write;
use std::sync::Arc;

use poise::serenity_prelude as serenity;
use tracing::{error, info};

use crate::database::models::meeting::Meeting;
use crate::database::models::member::Member;
use crate::database::models::summary::Summary;
use crate::discord::Context;
use crate::discord::Error;
use crate::meeting::MeetingStatus;

/// Ends the meeting. Returns the meeting summary which contains members' attendance and reports
#[poise::command(slash_command, rename = "end")]
pub(crate) async fn end_meeting(
    ctx: Context<'_>,
    #[description = "Note to add to the summary"]
    #[rest]
    note: String,
) -> Result<(), Error> {
    info!("Received end-meeting command");

    let meeting_status = ctx.data().meeting_status.clone();

    if !meeting_status.read().await.is_meeting_ongoing() {
        let error = "No meeting is ongoing".to_string();
        error!("{}", error);
        return Err(anyhow!(error));
    }

    let rw_lock_read_guard = meeting_status.read().await;

    let meeting = Meeting::find_by_id(rw_lock_read_guard.meeting_id())?;
    let mut summary = Summary::find_by_id(meeting.summary_id())?;

    summary.set_note(note.clone())?;
    let summary_result = summary.send_summary(ctx, false).await?;
    drop(rw_lock_read_guard);

    MeetingStatus::end_meeting(ctx.serenity_context(), meeting_status).await?;

    crate::discord::respond(ctx, summary_result).await
}

/// Return the current or future meeting's status.
#[poise::command(slash_command, rename = "status")]
pub(crate) async fn status_meeting(ctx: Context<'_>) -> Result<(), Error> {
    let mut output = String::new();

    info!("Received status-meeting command");

    let rw_lock = &ctx.data().meeting_status.clone();
    let meeting_status = rw_lock.read().await;

    if meeting_status.is_meeting_ongoing() {
        output.push_str("Meeting is ongoing. ");
        output.push_str(&meeting_status.meeting_id().simple().to_string());
    } else {
        output.push_str("Planned meeting on ");
        output.push_str(
            &meeting_status
                .schedule()?
                .upcoming(chrono::Local)
                .next()
                .unwrap()
                .to_string(),
        );
        output.push_str(" with id ");
        output.push_str(&meeting_status.meeting_id().simple().to_string());
    }

    output.push_str("\nMembers:");
    for member in meeting_status.members() {
        output.push_str(" <@");
        output.push_str(&member.discord_id()?);
        output.push('>');
    }

    output.push_str("\nMonitoring channel: <#");
    output.push_str(meeting_status.channel());
    output.push('>');

    info!("Generated meeting status: \n{}", output);

    crate::discord::respond(ctx, output).await
}

/// Change the meeting's details.
///
/// Edit the meeting's schedule and channel.
#[poise::command(slash_command, rename = "plan")]
pub(crate) async fn plan_meeting(
    ctx: Context<'_>,
    #[description = "Schedule of the meeting"] schedule: Option<cron::Schedule>,
    #[description = "Channel to monitor"]
    #[channel_types("Voice")]
    channel: Option<poise::serenity_prelude::GuildChannel>,
) -> Result<(), Error> {
    let mut output = String::new();

    let meeting_status = ctx.data().meeting_status.clone();

    if let Some(schedule) = schedule {
        let next = schedule.upcoming(chrono::Local).next().unwrap();

        MeetingStatus::change_schedule(
            Arc::clone(&meeting_status),
            &schedule.to_string(),
            ctx.serenity_context(),
        )
        .await?;
        output.push_str("New schedule set to ");
        output.push_str(&schedule.to_string());
        output.push_str(" (next meeting on ");
        output.push_str(&next.to_string());
        output.push(')');
    }

    if let Some(channel) = channel {
        let channel_id = channel.id;

        let mut meeting_status = meeting_status.write().await;

        match meeting_status.change_channel(channel_id.to_string()) {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error changing channel: {}", e);
                error!("{}", error);
                return Err(anyhow!(error));
            }
        }
        output.push_str("\nMeeting channel changed to <#");
        output.push_str(&channel_id.to_string());
        output.push('>');
    }

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "set-note")]
pub(crate) async fn set_note(
    ctx: Context<'_>,
    #[description = "Meeting ID to set the note for"] meeting: Option<Meeting>,
    #[description = "Note to set"]
    #[rest]
    note: String,
) -> Result<(), Error> {
    let mut output = String::new();

    let meeting_status = ctx.data().meeting_status.read().await;

    output.push_str("Meeting summary changed to ");
    output.push_str(&note);

    let meeting = match meeting {
        Some(meeting) => meeting,
        None => Meeting::find_by_id(meeting_status.meeting_id())?,
    };

    let mut summary = Summary::find_by_id(meeting.summary_id())?;

    match summary.set_note(note) {
        Ok(_) => {}
        Err(e) => {
            let error_msg = format!("Error changing summary note: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    }

    match summary.send_summary(ctx, true).await {
        Ok(_) => {}
        Err(e) => {
            let error = format!("Error sending summary: {}", e);
            error!("{}", error);
            return Err(anyhow!(error));
        }
    }

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "add-member")]
pub async fn add_member(
    ctx: Context<'_>,
    #[description = "Member to add"] member: serenity::Member,
    #[description = "Meeting ID to add the member to"] meeting: Option<Meeting>,
) -> Result<(), Error> {
    info!("Adding member to meeting");
    let mut output = String::new();

    let member = match Member::from_discord_id(member.user.id.to_string(), ctx) {
        Ok(member) => member,
        Err(e) => {
            let error_msg = format!("Error finding member: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    };

    let result = match meeting {
        Some(meeting) => meeting.add_member(&member)?,
        None => {
            let mut meeting_status = ctx.data().meeting_status.write().await;
            meeting_status.add_member(&member)?
        }
    };

    output.push_str(&result);

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "remove-member")]
pub async fn remove_member(
    ctx: Context<'_>,
    #[description = "Member to remove"] member: serenity::Member,
    #[description = "Meeting ID to add the member to"] meeting: Option<Meeting>,
) -> Result<(), Error> {
    info!("Removing member from meeting");
    let mut output = String::new();

    let member = match Member::from_discord_id(member.user.id.to_string(), ctx) {
        Ok(member) => member,
        Err(e) => {
            let error_msg = format!("Error finding member: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    };

    let result = match meeting {
        Some(meeting) => meeting.remove_member(&member)?,
        None => {
            let mut meeting_status = ctx.data().meeting_status.write().await;
            meeting_status.remove_member(&member)?
        }
    };

    output.push_str(&result);

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "list")]
pub(crate) async fn list_meetings(
    ctx: Context<'_>,
    #[description = "Page to list"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
) -> Result<(), Error> {
    info!("Listing meetings");

    let page = page.unwrap_or(1);

    let (meetings, total_pages) = Meeting::list(page, page_size)?;

    let mut output = String::new();

    for meeting in meetings {
        writeln!(&mut output, "{}\n", meeting)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    crate::discord::respond(ctx, output).await
}
