use std::fmt::Write;
use std::{str::FromStr, sync::Arc};

use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use tracing::{error, info};
use uuid::Uuid;

use crate::database::models::meeting::Meeting;
use crate::database::models::member::Member;
use crate::database::models::summary::Summary;
use crate::discord::{find_option_as_string, find_option_value};
use crate::meeting::MeetingStatus;

/// Ends the meeting. Returns the meeting summary, containing the meeting's members, their attendance and reports
pub(crate) async fn end_meeting(
    option: &CommandDataOption,
    ctx: &Context,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Received end-meeting command");

    let note = find_option_as_string(&option.options[..], "note").unwrap_or_default();

    let read = ctx.data.read().await;
    let meeting_status = read.get::<MeetingStatus>().unwrap().clone();

    if !meeting_status.read().await.is_meeting_ongoing() {
        let error = "No meeting is ongoing".to_string();
        error!("{}", error);
        return Err(error.into());
    }

    let rw_lock_read_guard = meeting_status.read().await;

    let meeting = Meeting::find_by_id(rw_lock_read_guard.meeting_id())?;
    let mut summary = Summary::find_by_id(meeting.summary_id())?;

    summary.set_note(note.clone())?;
    let summary_result = summary.send_summary(ctx, false).await?;
    drop(rw_lock_read_guard);

    MeetingStatus::end_meeting(ctx, meeting_status).await?;

    Ok(summary_result)
}

/// Return the current or future meeting status.
pub(crate) async fn status_meeting(ctx: &Context) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();

    info!("Received status-meeting command");

    let data_read = ctx.data.read().await;
    let meeting_status = data_read.get::<MeetingStatus>().unwrap().clone();
    let meeting_status = meeting_status.read().await;

    if meeting_status.is_meeting_ongoing() {
        output.push_str("Meeting is ongoing. ");
        output.push_str(&meeting_status.meeting_id().to_string());
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
        output.push_str(&meeting_status.meeting_id().to_string());
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

    Ok(output)
}

/// Change the meeting's details.
///
/// Edit the meeting's schedule and channel.
pub(crate) async fn plan_meeting(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();

    let data_read = ctx.data.read().await;
    let meeting_status = data_read.get::<MeetingStatus>().unwrap().clone();
    let meeting_status = meeting_status.clone();

    if let Some(new_schedule) = find_option_as_string(&option.options, "schedule") {
        // check if the schedule is valid
        let schedule = match cron::Schedule::from_str(&new_schedule) {
            Ok(schedule) => schedule,
            Err(e) => {
                error!("Invalid schedule: {}", e);
                return Err("Invalid schedule".into());
            }
        };
        let next = schedule.upcoming(chrono::Local).next().unwrap();

        MeetingStatus::change_schedule(Arc::clone(&meeting_status), &new_schedule, ctx)
            .await
            .unwrap();
        output.push_str("New schedule set to ");
        output.push_str(&new_schedule);
        output.push_str(" (next meeting on ");
        output.push_str(&next.to_string());
        output.push(')');
    }

    if let Some(new_channel) = find_option_as_string(&option.options, "channel") {
        let channel_id = new_channel.parse::<u64>().unwrap();
        let channel = ctx.cache.guild_channel(channel_id).unwrap();

        // check if channel is voice channel
        if channel.kind != serenity::model::channel::ChannelType::Voice {
            error!("Channel is not a voice channel: {}", channel_id);
            return Err("Channel is not a voice channel".into());
        }

        let mut meeting_status = meeting_status.write().await;

        match meeting_status.change_channel(channel_id.to_string()) {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error changing channel: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        }
        output.push_str("\nMeeting channel changed to <#");
        output.push_str(&channel_id.to_string());
        output.push('>');
    }

    Ok(output)
}

pub(crate) async fn set_note(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();

    let data_read = ctx.data.read().await;
    let meeting_status = data_read.get::<MeetingStatus>().unwrap().clone();
    let meeting_status = meeting_status.clone();
    let meeting_status = meeting_status.read().await;

    if let Some(new_note) = find_option_as_string(&option.options, "note") {
        output.push_str("Meeting summary changed to ");
        output.push_str(&new_note);

        let meeting = if let Some(meeting) = find_option_as_string(&option.options, "meeting-id") {
            let meeting_id = Uuid::parse_str(&meeting)?;
            Meeting::find_by_id(meeting_id)?
        } else {
            Meeting::find_by_id(meeting_status.meeting_id())?
        };

        let mut summary = Summary::find_by_id(meeting.summary_id())?;

        match summary.set_note(new_note) {
            Ok(_) => {}
            Err(e) => {
                let error_msg = format!("Error changing summary note: {}", e);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        }

        match summary.send_summary(ctx, true).await {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error sending summary: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        }
    } else {
        output.push_str("Meeting summary unchanged");
    };
    Ok(output)
}

pub(crate) async fn edit_meeting_members(
    ctx: &Context,
    option: &CommandDataOption,
    remove: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    info!(remove, ?option, "Adding/Removing members from meeting");
    let mut output = String::new();

    if let Some(member) = find_option_as_string(&option.options, "member") {
        let member = Member::from_discord_id(member, ctx)?;

        if let Some(meeting) = find_option_as_string(&option.options, "meeting-id") {
            let meeting_id = match Uuid::parse_str(&meeting) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Invalid meeting id: {}\nReason: {}", meeting, why);
                    error!("{}", error_msg);
                    return Err(error_msg.into());
                }
            };

            let meeting = match Meeting::find_by_id(meeting_id) {
                Ok(meeting) => meeting,
                Err(why) => {
                    let error_msg = format!(
                        "Meeting not found in database: {}\nReason: {}",
                        meeting_id, why
                    );
                    error!("{}", error_msg);
                    return Err(error_msg.into());
                }
            };

            if remove {
                output.push_str(&meeting.remove_member(&member)?);
            } else {
                output.push_str(&meeting.add_member(&member)?);
            }
        } else {
            let data_read = ctx.data.read().await;
            let meeting_status = data_read.get::<MeetingStatus>().unwrap().clone();
            let mut meeting_status = meeting_status.write().await;

            if remove {
                output.push_str(&meeting_status.remove_member(&member).unwrap());
            } else {
                output.push_str(&meeting_status.add_member(&member).unwrap());
            }
        }
    } else {
        output.push_str("No member specified");
    }

    Ok(output)
}

pub(crate) async fn list_meetings(
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing meetings");
    let page =
        find_option_value(&option.options[..], "page").map_or(1, |page| page.as_i64().unwrap());

    let page_size = find_option_value(&option.options[..], "page-size")
        .map(|page_size| page_size.as_i64().unwrap());

    let (meetings, total_pages) = Meeting::list(page, page_size)?;

    let mut output = String::new();

    for meeting in meetings {
        writeln!(&mut output, "{}\n", meeting)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    Ok(output)
}
