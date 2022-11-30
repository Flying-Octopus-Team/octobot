use std::fmt::Write;
use std::str::FromStr;
use std::sync::Arc;

use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use serenity::prelude::Mentionable;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::database::models::meeting::MeetingFilter;
use crate::discord::find_option_as_string;
use crate::discord::find_option_value;
use crate::framework;
use crate::framework::meeting::Meeting;
use crate::framework::member::Member;
use crate::framework::summary::SummaryBuilder;
use crate::meeting::MeetingStatus;

/// Ends the meeting. Returns the meeting summary, containing the meeting's members, their attendance and reports
pub(crate) async fn end_meeting(
    ctx: Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Received end-meeting command");

    let note = find_option_as_string(&option.options[..], "note").unwrap_or_default();

    match Meeting::end(note, &ctx).await {
        Ok(output) => Ok(output),
        Err(e) => {
            error!("Error ending meeting: {}", e);
            Err("Error ending meeting".into())
        }
    }
}

/// Return the current or future meeting status.
pub(crate) async fn status_meeting(ctx: Context) -> Result<String, Box<dyn std::error::Error>> {
    info!("Received status-meeting command");

    let output = framework::meeting::status(&ctx).await;

    info!("Generated meeting status: \n{}", output);

    Ok(output)
}

/// Change the meeting's details.
///
/// Edit the meeting's schedule and channel.
pub(crate) async fn plan_meeting(
    ctx: Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();

    let data_read = ctx.data.read().await;
    let meeting_status = data_read.get::<MeetingStatus>().unwrap();

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

        MeetingStatus::change_schedule(Arc::clone(meeting_status), &new_schedule, &ctx)
            .await
            .unwrap();

        write!(
            output,
            "New schedule set to {new_schedule} (next meeting on {next})"
        )?;
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

        write!(output, "\nMeeting channel changed to {}", channel.mention())?;
    }

    Ok(output)
}

pub(crate) async fn set_note(
    ctx: Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut output = String::new();

    if let Some(new_note) = find_option_as_string(&option.options, "note") {
        output.push_str("Meeting summary changed to ");
        output.push_str(&new_note);

        let mut meeting =
            if let Some(meeting) = find_option_as_string(&option.options, "meeting-id") {
                let meeting_id = Uuid::parse_str(&meeting)?;
                Meeting::get(&ctx, meeting_id).await?
            } else {
                Meeting::get_current_meeting(&ctx).await
            };

        let summary_builder = SummaryBuilder::new().note(new_note.clone());
        meeting.summary.edit(summary_builder).await?;

        match meeting.resend_summary(&ctx).await {
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
    ctx: Context,
    option: &CommandDataOption,
    remove: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    info!(remove, ?option, "Adding/Removing members from meeting");
    let mut output = String::new();

    if let Some(member) = find_option_as_string(&option.options, "member") {
        let member = Member::get_by_discord_id(member.parse().unwrap(), &ctx)
            .await?
            .unwrap();

        if let Some(meeting) = find_option_as_string(&option.options, "meeting-id") {
            let meeting_id = match Uuid::parse_str(&meeting) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Invalid meeting id: {}\nReason: {}", meeting, why);
                    error!("{}", error_msg);
                    return Err(error_msg.into());
                }
            };

            let mut meeting = match Meeting::get(&ctx, meeting_id).await {
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
                output.push_str(&meeting.remove_member(member).await?);
            } else {
                output.push_str(&meeting.add_member(member).await?);
            }
        } else {
            let mut meeting = Meeting::get_current_meeting(&ctx).await;

            if remove {
                output.push_str(&meeting.remove_member(member).await.unwrap());
            } else {
                output.push_str(&meeting.add_member(member).await.unwrap());
            }
        }
    } else {
        output.push_str("No member specified");
    }

    Ok(output)
}

pub(crate) async fn list_meetings(
    ctx: Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing meetings");
    let page =
        find_option_value(&option.options[..], "page").map_or(1, |page| page.as_i64().unwrap());

    let page_size = find_option_value(&option.options[..], "page-size")
        .map(|page_size| page_size.as_i64().unwrap());

    let filter = MeetingFilter::new();
    let (meetings, total_pages) = Meeting::list(filter, &ctx, page, page_size).await?;

    let mut output = String::new();

    for meeting in meetings {
        writeln!(&mut output, "{}", meeting)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    Ok(output)
}
