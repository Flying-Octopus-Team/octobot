use std::{fmt::Write, sync::Arc, time::Duration};

use poise::serenity_prelude::{ComponentInteractionCollector, CreateActionRow, CreateButton};
use tracing::info;

use crate::{
    database::models::{
        meeting::Meeting,
        member::{Member, MemberRole},
        summary::Summary,
    },
    discord::Context,
    error::Error,
    meeting::MeetingStatus,
    SETTINGS,
};

/// Ends the meeting. Returns the meeting summary which contains members'
/// attendance and reports
#[poise::command(slash_command, rename = "end")]
pub(crate) async fn end_meeting(
    ctx: Context<'_>,
    #[description = "Note to add to the summary"]
    #[rest]
    note: String,
) -> Result<(), Error> {
    let meeting_status = ctx.data().meeting_status.clone();

    if !meeting_status.read().await.is_meeting_ongoing() {
        return Err(Error::NoMeetingOngoing);
    }

    let mut summary_result;

    {
        let rw_lock_read_guard = meeting_status.read().await;

        let meeting = Meeting::find_by_id(rw_lock_read_guard.meeting_id())?;
        let mut summary = Summary::find_by_id(meeting.summary_id())?;

        summary.set_note(note.clone())?;
        summary_result = summary.send_summary(ctx, false).await?;
    }

    let mut page = 1;

    let previous_meeting = Meeting::get_previous_meeting()?;

    info!(
        "Active after: {:?}",
        previous_meeting.start_date() - chrono::Duration::days(SETTINGS.activity_threshold_days)
    );

    let (members, total_pages) = Member::list(
        page,
        None,
        None,
        Some(MemberRole::ExMember),
        Some(crate::database::models::member::Activity::Inactive),
        Some(
            (previous_meeting.start_date()
                - chrono::Duration::days(SETTINGS.activity_threshold_days))
            .into(),
        ),
    )?;

    if !members.is_empty() {
        summary_result.push_str("\nInactive members from this week:");
    }

    for member in members {
        summary_result.push('\n');
        summary_result.push_str(&member.display_activity());
    }

    page += 1;

    while page <= total_pages {
        let (members, _) = Member::list(
            page,
            None,
            None,
            Some(MemberRole::ExMember),
            Some(crate::database::models::member::Activity::Inactive),
            Some((chrono::Local::now().naive_local() - chrono::Duration::weeks(1)).into()),
        )?;

        for member in members {
            summary_result.push('\n');
            summary_result.push_str(&member.display_activity());
        }

        page += 1;
    }

    MeetingStatus::end_meeting(ctx.serenity_context(), meeting_status).await?;

    crate::discord::respond(ctx, summary_result).await
}

/// Return the current or future meeting's status.
#[poise::command(slash_command, rename = "status")]
pub(crate) async fn status_meeting(ctx: Context<'_>) -> Result<(), Error> {
    let mut output = String::new();

    {
        let rw_lock = ctx.data().meeting_status.clone();
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
    }

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
        MeetingStatus::change_schedule(
            Arc::clone(&meeting_status),
            schedule.clone(),
            ctx.serenity_context(),
        )
        .await?;

        let next = schedule.upcoming(chrono::Local).next().unwrap();

        output.push_str("New schedule set to ");
        output.push_str(&schedule.to_string());
        output.push_str(" (next meeting on ");
        output.push_str(&next.to_string());
        output.push(')');
    }

    if let Some(channel) = channel {
        let channel_id = channel.id;

        let mut meeting_status = meeting_status.write().await;

        meeting_status.change_channel(channel_id.to_string())?;

        output.push_str("\nMeeting channel changed to <#");
        output.push_str(&channel_id.to_string());
        output.push('>');
    }

    crate::discord::respond(ctx, output).await
}

/// Resolves the target meeting for note-related commands: the explicitly
/// given meeting, or the current one (ongoing or next planned) otherwise.
async fn resolve_meeting(ctx: Context<'_>, meeting: Option<Meeting>) -> Result<Meeting, Error> {
    match meeting {
        Some(meeting) => Ok(meeting),
        None => {
            let meeting_status = ctx.data().meeting_status.read().await;

            Meeting::find_by_id(meeting_status.meeting_id())
        }
    }
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

    output.push_str("Meeting summary changed to ");
    output.push_str(&note);

    let meeting = resolve_meeting(ctx, meeting).await?;

    let mut summary = Summary::find_by_id(meeting.summary_id())?;

    summary.set_note(note)?;

    summary.send_summary(ctx, true).await?;

    crate::discord::respond(ctx, output).await
}

/// Modal used to compose a meeting note. A modal's paragraph input allows
/// comfortable multi-line text entry, unlike a slash command string option.
#[derive(Debug, poise::Modal)]
#[name = "Compose meeting note"]
struct NoteModal {
    #[name = "Title (optional)"]
    #[placeholder = "e.g. Sprint retro highlights"]
    #[max_length = 100]
    title: Option<String>,
    #[name = "Note"]
    #[placeholder = "What should be recorded for this meeting?"]
    #[paragraph]
    #[min_length = 1]
    #[max_length = 3900]
    note: String,
}

fn compose_note_button() -> CreateButton {
    CreateButton::new("compose_note_button").label("Compose note")
}

/// Formats the modal's title/note fields into the text stored on the
/// summary. Returns [`Error::EmptyNote`] if the note is blank once
/// whitespace is trimmed off both ends (Discord only guarantees the field is
/// non-empty *before* trimming, so a whitespace-only submission is still
/// possible).
fn format_note(title: Option<String>, note: String) -> Result<String, Error> {
    let note = note.trim();

    if note.is_empty() {
        return Err(Error::EmptyNote);
    }

    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());

    Ok(match title {
        Some(title) => format!("**{title}**\n\n{note}"),
        None => note.to_string(),
    })
}

/// Opens a pop-up form (Discord modal) to compose the meeting's note.
///
/// Unlike `set-note`, the note is entered in a multi-line text box, which is
/// friendlier for longer or multi-paragraph notes than a single slash
/// command option.
#[poise::command(slash_command, rename = "compose-note")]
pub(crate) async fn compose_note(
    ctx: Context<'_>,
    #[description = "Meeting ID to set the note for (defaults to the current meeting)"]
    meeting: Option<Meeting>,
) -> Result<(), Error> {
    let meeting = resolve_meeting(ctx, meeting).await?;

    ctx.send(
        poise::CreateReply::default()
            .content("Click the button below to open the note form.")
            .components(vec![CreateActionRow::Buttons(vec![compose_note_button()])]),
    )
    .await?;

    let interaction = ComponentInteractionCollector::new(ctx.serenity_context())
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .filter(|interaction| interaction.data.custom_id == "compose_note_button")
        .timeout(Duration::from_secs(600))
        .await;

    let Some(interaction) = interaction else {
        return crate::discord::respond(
            ctx,
            "Timed out waiting for the note form to be opened.".to_string(),
        )
        .await;
    };

    let modal_data = poise::execute_modal_on_component_interaction::<NoteModal>(
        &ctx,
        interaction,
        None,
        Some(Duration::from_secs(600)),
    )
    .await?;

    let Some(modal_data) = modal_data else {
        return crate::discord::respond(
            ctx,
            "Timed out waiting for the note to be submitted.".to_string(),
        )
        .await;
    };

    let note = format_note(modal_data.title, modal_data.note)?;

    let mut summary = Summary::find_by_id(meeting.summary_id())?;

    summary.set_note(note.clone())?;

    let result = summary.send_summary(ctx, true).await?;

    crate::discord::respond(ctx, format!("Meeting summary changed to {note}\n{result}")).await
}

#[poise::command(slash_command, rename = "add-member")]
pub async fn add_member(
    ctx: Context<'_>,
    #[description = "Member to add"] mut member: Member,
    #[description = "Meeting ID to add the member to"] meeting: Option<Meeting>,
) -> Result<(), Error> {
    let mut output = String::new();

    let result = match meeting {
        Some(meeting) => meeting.add_member(&mut member)?,
        None => {
            let mut meeting_status = ctx.data().meeting_status.write().await;
            meeting_status.add_member(&mut member)?
        }
    };

    output.push_str(&result);

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "remove-member")]
pub async fn remove_member(
    ctx: Context<'_>,
    #[description = "Member of the organization"] mut member: Member,
    #[description = "Meeting ID to add the member to"] meeting: Option<Meeting>,
) -> Result<(), Error> {
    let mut output = String::new();

    let result = match meeting {
        Some(meeting) => meeting.remove_member(&mut member)?,
        None => {
            let mut meeting_status = ctx.data().meeting_status.write().await;
            meeting_status.remove_member(&mut member)?
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
    let page = page.unwrap_or(1);

    let (meetings, total_pages) = Meeting::list(page, page_size)?;

    let mut output = String::new();

    for meeting in meetings {
        writeln!(&mut output, "{}\n", meeting)?;
    }
    write!(output, "Page {}/{}", page, total_pages)?;

    crate::discord::respond(ctx, output).await
}

#[cfg(test)]
mod tests {
    use super::format_note;
    use crate::error::Error;

    #[test]
    fn note_without_title_is_passed_through() {
        let note = format_note(None, "Discussed the roadmap.".to_string()).unwrap();

        assert_eq!(note, "Discussed the roadmap.");
    }

    #[test]
    fn note_with_title_is_prefixed() {
        let note = format_note(
            Some("Retro".to_string()),
            "Discussed the roadmap.".to_string(),
        )
        .unwrap();

        assert_eq!(note, "**Retro**\n\nDiscussed the roadmap.");
    }

    #[test]
    fn note_and_title_are_trimmed() {
        let note = format_note(
            Some("  Retro  ".to_string()),
            "  Discussed the roadmap.  ".to_string(),
        )
        .unwrap();

        assert_eq!(note, "**Retro**\n\nDiscussed the roadmap.");
    }

    #[test]
    fn blank_title_is_treated_as_absent() {
        let note = format_note(
            Some("   ".to_string()),
            "Discussed the roadmap.".to_string(),
        )
        .unwrap();

        assert_eq!(note, "Discussed the roadmap.");
    }

    #[test]
    fn empty_note_is_rejected() {
        let err = format_note(None, String::new()).unwrap_err();

        assert!(matches!(err, Error::EmptyNote));
    }

    #[test]
    fn whitespace_only_note_is_rejected() {
        let err = format_note(None, "   \n\t  ".to_string()).unwrap_err();

        assert!(matches!(err, Error::EmptyNote));
    }
}
