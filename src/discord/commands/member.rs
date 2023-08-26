use std::fmt::Write;

use poise::serenity_prelude as serenity;
use tracing::error;
use tracing::info;

use super::Context;
use super::Error;
use crate::database::models::member::Member;
use crate::database::models::member::MemberRole;
use crate::SETTINGS;

#[poise::command(slash_command, rename = "add")]
pub async fn add_member(
    ctx: Context<'_>,
    #[description = "Discord account"] member: serenity::Member,
    #[description = "Trello ID account"] trello_id: Option<String>,
    #[description = "Trello Report Card ID"] trello_report_card_id: Option<String>,
    #[description = "Wiki ID"] wiki_id: Option<i64>,
    #[description = "Display name"] name: Option<String>,
    #[description = "Role"] role: Option<MemberRole>,
) -> Result<(), Error> {
    info!("Adding member");

    let mut output = String::new();

    let name = if let Some(name) = name {
        name
    } else {
        member.user.name
    };

    let discord_id = member.user.id;

    // check if member is already in the database
    if let Ok(member) = Member::find_by_discord_id(discord_id.to_string()) {
        let error_msg = format!("Member already exists in the database: {:?}", member);

        error!("{}", error_msg);

        output.push_str(&(error_msg + "\n"));

        return Ok(());
    }

    let member = Member::new(
        name,
        Some(discord_id.to_string()),
        trello_id,
        trello_report_card_id,
        role.unwrap_or_default(),
        wiki_id,
    );
    match member.role().add_role(&ctx, discord_id.0).await {
        Ok(_) => (),
        Err(why) => {
            let error_msg = format!("Failed to add role: {}", why);
            error!("{}", error_msg);
            output.push_str(&(error_msg + "\n"));
        }
    }

    if member.wiki_id().is_some() {
        member.assign_wiki_group_by_role().await?;
    }

    let member = match member.insert() {
        Ok(member) => member,
        Err(e) => {
            let error_msg = format!("Failed to insert member into database: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    };

    info!("Member added: {:?}", member);

    output.push_str(&format!("Added {}", member));

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "remove")]
pub async fn remove_member(
    ctx: Context<'_>,
    #[description = "Member ID"] member: crate::database::models::member::Member,
    #[description = "Hard delete member from the database"] hard_delete: Option<bool>,
) -> Result<(), Error> {
    info!("Removing member");
    let mut output = String::new();

    if let Some(user_id) = member.discord_id() {
        let user_id = user_id.parse().unwrap();

        match member.role().remove_role(&ctx, user_id).await {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to remove member role: {}", why);
                error!("{}", error_msg);
                output.push_str(&error_msg);
                output.push('\n');
            }
        }
    }

    if member.wiki_id().is_some() {
        member
            .unassign_wiki_group(SETTINGS.wiki.member_group_id)
            .await?;

        if let Err(why) = member.assign_wiki_group(SETTINGS.wiki.guest_group_id).await {
            let error_msg = format!("Failed to assign wiki group: {}", why);
            error!("{}", error_msg);
            output.push_str(&(error_msg + "\n"));
        }
    }

    if hard_delete.unwrap_or(false) {
        member.hard_delete()?;
    } else {
        member.delete()?;
    }

    info!("Member removed: {:?}", member);

    output.push_str(&format!("Removed {}", member));

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "update")]
#[allow(clippy::too_many_arguments)]
pub async fn update_member(
    ctx: Context<'_>,
    #[description = "Member ID"] mut member: crate::database::models::member::Member,
    #[description = "Member name"] name: Option<String>,
    #[description = "Member Discord"] discord_member: Option<serenity::Member>,
    #[description = "Member Trello ID"] trello_id: Option<String>,
    #[description = "Member Trello Report Card ID"] trello_report_card_id: Option<String>,
    #[description = "Member role"] role: Option<MemberRole>,
    #[description = "Member wiki ID"] wiki_id: Option<i64>,
) -> Result<(), Error> {
    info!("Updating member");

    let mut output = String::new();

    if let Some(new_name) = name {
        match member.set_name(new_name) {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to update member name: {}", why);
                error!("{}", error_msg);
                return Err(anyhow!(error_msg));
            }
        }
    }

    if let Some(new_discord_member) = discord_member {
        let dc_id = new_discord_member.user.id.0;

        if let Some(old_dc_id) = member.discord_id() {
            match member.role().remove_role(&ctx, old_dc_id.parse()?).await {
                Ok(_) => {}
                Err(why) => {
                    let error_msg = format!("Failed to remove member role: {}", why);
                    error!("{}", error_msg);
                    output.push_str(&error_msg);
                    output.push('\n');
                }
            }
        }

        member.set_discord_id(dc_id.to_string());
    }

    if let Some(new_role) = role {
        let user_id = member.discord_id().unwrap().parse()?;

        match member.role().remove_role(&ctx, user_id).await {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to remove member role: {}", why);
                error!("{}", error_msg);
                output.push_str(&error_msg);
                output.push('\n');
            }
        }

        member.set_role(new_role);
    }

    if let Some(dc_id) = member.discord_id() {
        member.role().add_role(&ctx, dc_id.parse::<u64>()?).await?;
    }

    if let Some(new_trello_id) = trello_id {
        member.set_trello_id(new_trello_id)
    }

    if let Some(new_trello_report_card_id) = trello_report_card_id {
        member.set_trello_report_card_id(new_trello_report_card_id)
    }

    if let Some(new_wiki_id) = wiki_id {
        member.set_wiki_id(new_wiki_id)
    }

    match member.update() {
        Ok(_) => {}
        Err(e) => {
            let error_msg = format!("Failed to update member in database: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    }

    info!("Member updated: {}", member);

    output.push_str(&format!("Updated {}", member));

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "list")]
pub async fn list_members(
    ctx: Context<'_>,
    #[description = "Page number"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
) -> Result<(), Error> {
    info!("Listing members");
    let page = page.unwrap_or(1);

    let (members, total_pages) = Member::list(page, page_size)?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}\n", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    crate::discord::respond(ctx, output).await
}
