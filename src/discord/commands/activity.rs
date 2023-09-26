use std::fmt::Write;

use crate::{
    database::models::member::{Activity, Member, MemberRole},
    discord::Context,
    error::Error,
};

#[poise::command(slash_command, rename = "refresh")]
pub(crate) async fn refresh(ctx: Context<'_>) -> Result<(), Error> {
    Member::refresh_all_activities()?;

    crate::discord::respond(ctx, "Refreshed".to_owned()).await
}

#[poise::command(slash_command, rename = "list")]
pub(crate) async fn list(
    ctx: Context<'_>,
    #[description = "Page to list"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
    #[description = "Member's activity"] activity: Option<Activity>,
) -> Result<(), Error> {
    let page = page.unwrap_or(1);

    let activity = activity.unwrap_or(Activity::Inactive);

    let (members, total_pages) = Member::list(
        page,
        page_size,
        None,
        Some(MemberRole::ExMember),
        Some(activity),
    )?;

    let mut output = String::new();

    write!(
        &mut output,
        "## List of {} members\n",
        activity.to_string().to_lowercase()
    )?;

    for member in members {
        let user_name = member
            .discord_id()
            .map(|id| format!("<@{}>", id))
            .unwrap_or_else(|| member.name().to_owned());

        let last_activity = member
            .last_activity()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "Never".to_owned());

        writeln!(
            &mut output,
            "{} {} Last active: {}",
            member.role(),
            user_name,
            last_activity
        )?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    crate::discord::respond(ctx, output).await
}
