use std::fmt::Write;

use serenity::model::prelude::interaction::application_command::CommandDataOption;
use serenity::prelude::Context;
use serenity::prelude::Mentionable;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::discord::find_option_as_string;
use crate::discord::find_option_value;
use crate::framework;
use crate::framework::meeting::Meeting;
use crate::framework::member::Member;

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

    if let Some(new_schedule) = find_option_as_string(&option.options, "schedule") {
        Meeting::change_future_schedule(&ctx, &*new_schedule).await?;

        write!(output, "New schedule set to")?;
    }

    if let Some(new_channel) = find_option_as_string(&option.options, "channel") {
        let channel_id = new_channel.parse::<u64>().unwrap();
        let channel = ctx.cache.guild_channel(channel_id).unwrap();

        write!(output, "\nMeeting channel changed to {}", channel.mention())?;

        match Meeting::change_future_channel(&ctx, channel).await {
            Ok(_) => {}
            Err(e) => {
                let error = format!("Error changing channel: {}", e);
                error!("{}", error);
                return Err(error.into());
            }
        }
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

        meeting.summary.note = new_note;

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

    let mut meeting: Meeting;

    if let Some(member) = find_option_as_string(&option.options, "member") {
        let member = Member::get_by_discord_id(member.parse().unwrap(), &ctx)
            .await?
            .unwrap();

        if let Some(meeting_id) = find_option_as_string(&option.options, "meeting-id") {
            let meeting_id = match Uuid::parse_str(&meeting_id) {
                Ok(id) => id,
                Err(why) => {
                    let error_msg = format!("Invalid meeting id: {}\nReason: {}", meeting_id, why);
                    error!("{}", error_msg);
                    return Err(error_msg.into());
                }
            };

            meeting = match Meeting::get(&ctx, meeting_id).await {
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
        } else {
            meeting = Meeting::get_current_meeting(&ctx).await;
        }

        if remove {
            output.push_str(&meeting.remove_member(member).await.unwrap());
        } else {
            output.push_str(&meeting.add_member(member).await.unwrap());
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

    let (meetings, total_pages) = Meeting::find().list(&ctx, page, page_size).await?;

    let mut output = String::new();

    for meeting in meetings {
        writeln!(&mut output, "{}", meeting)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    Ok(output)
}
