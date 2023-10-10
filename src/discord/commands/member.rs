use std::{fmt::Write, future::IntoFuture};

use poise::serenity_prelude::{
    self as serenity, CreateActionRow, CreateButton, CreateMessage, EditMessage,
};
use tracing::{error, info, warn};

use super::Context;
use crate::{
    database::models::member::{Activity, Member, MemberRole},
    error::Error,
    SETTINGS,
};

/// Modal to get the Discord email of a user to connect or create a new wiki
/// account
#[derive(Debug, poise::Modal)]
#[name = "Discord email for Wiki account"]
struct WikiEmailModal {
    #[name = "Discord email"]
    #[placeholder = "example@domain.com"]
    #[min_length = 1]
    #[max_length = 254]
    wiki_email: String,
}

fn discord_email_button() -> CreateButton {
    CreateButton::new("discord_email_button").label("Discord email")
}

fn dm_wiki_details() -> CreateMessage {
    CreateMessage::new().content("Welcome to Flying Octopus! In order to create your account on our wiki, please provide your Discord email address (the one you use to log into Discord).")
        .components(
            vec![CreateActionRow::Buttons(vec![discord_email_button()])]
        )
}

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
    let name = if let Some(name) = name {
        name
    } else {
        member.display_name().to_string()
    };

    let discord_id = member.user.id;
    let mut output = String::new();

    // check if member is already in the database
    if let Ok(member) = Member::find_by_discord_id(discord_id.to_string()) {
        let error_msg = format!("Member already exists in the database: {:?}", member);

        error!("{}", error_msg);

        output.push_str(&(error_msg + "\n"));

        crate::discord::respond(ctx, output).await?;

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

    match member.role().add_role(&ctx, discord_id.get()).await {
        Ok(_) => (),
        Err(why) => {
            let error_msg = format!("Failed to add role: {}", why);
            error!("{}", error_msg);
            output.push_str(&(error_msg + "\n"));
        }
    }

    let ask_wiki_details = member.wiki_id().is_none();

    if !ask_wiki_details {
        member.assign_wiki_group_by_role().await?;
    }

    let mut member = member.insert()?;

    info!("Member added: {:?}", member);
    output.push_str(&format!("Added {}", member));

    if ask_wiki_details {
        output.push_str("\nInstructions to create wiki account sent via DM to the new member.");
    }

    crate::discord::respond(ctx, output).await?;

    if ask_wiki_details {
        // DM new member to get either their Discord email to create new wiki account or
        // their wiki ID
        let dm = discord_id.create_dm_channel(&ctx).await?;

        let mut msg = dm.send_message(&ctx, dm_wiki_details()).await?;

        let mut wiki_id = None;

        let mut response =
            poise::serenity_prelude::ComponentInteractionCollector::new(ctx.serenity_context())
                .channel_id(dm.id)
                .author_id(discord_id)
                .message_id(msg.id)
                .timeout(std::time::Duration::from_secs(3600))
                .await;

        while let Some(interaction) = response.clone() {
            info!("User {} responded to wiki email modal", member.name());
            let collector =
                poise::serenity_prelude::ComponentInteractionCollector::new(ctx.serenity_context())
                    .channel_id(dm.id)
                    .author_id(discord_id)
                    .message_id(msg.id)
                    .timeout(std::time::Duration::from_secs(3600))
                    .into_future();

            let email = tokio::select! {
                email = poise::execute_modal_on_component_interaction::<WikiEmailModal>(&ctx, std::sync::Arc::new(interaction), None, Some(std::time::Duration::from_secs(3600))) => email?,
                interaction =
                collector
                => {
                    response = interaction;
                    continue;
                }
            };

            if let Some(email) = email {
                let email = email.wiki_email;

                let user_id = crate::wiki::find_or_create_user(email, member.name().clone())
                    .await
                    .unwrap();

                wiki_id = Some(user_id);

                let _ = msg
                    .edit(
                        &ctx,
                        EditMessage::new()
                            .content(format!(
                                "Your wiki account has been created. You can now login at {}",
                                SETTINGS.wiki.url
                            ))
                            .components(Vec::new()),
                    )
                    .await;

                break;
            } else {
                // Timeout
                warn!("User {} did not respond to wiki email modal", member.name());

                let _ = msg
                    .edit(&ctx,
                EditMessage::new()
                        .content("Timed out waiting for response. Please use `/member update` to update your wiki ID.")
                        .components(Vec::new())
                    )
                    .await;

                break;
            }
        }

        if response.is_none() {
            // Timeout
            warn!("User {} did not respond to wiki email modal", member.name());

            // Edit message to indicate timeout
            let _ = msg
                .edit(&ctx,
            EditMessage::new()
                    .content("Timed out waiting for response. Please use `/member update` to update your wiki ID.")
                    // .components(|c| c)
            )
                .await;

            return Ok(());
        }

        if let Some(wiki_id) = wiki_id {
            // Get member again to make sure we have the latest version
            member = Member::find_by_discord_id(discord_id.to_string())?;

            member.set_wiki_id(wiki_id);

            member
                .unassign_wiki_group(SETTINGS.wiki.guest_group_id)
                .await?;

            member.assign_wiki_group_by_role().await?;

            member.update()?;
        }
    }

    Ok(())
}

#[poise::command(slash_command, rename = "remove")]
pub async fn remove_member(
    ctx: Context<'_>,
    #[description = "Member of the organization"] member: Member,
    #[description = "Hard delete member from the database"] hard_delete: Option<bool>,
) -> Result<(), Error> {
    let mut output = String::new();

    if let Some(user_id) = member.discord_id() {
        let user_id = user_id.parse().unwrap();

        match member.role().remove_role(&ctx, user_id).await {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to remove member's role: {}", why);
                error!("{}", error_msg);
                output.push_str(&error_msg);
                output.push('\n');
            }
        }
    }

    if member.wiki_id().is_some() {
        member.unassign_wiki_group_by_role().await?;

        // assign account to guest group
        crate::wiki::assign_user_group(crate::wiki::assign_user_group::Variables {
            user_id: member.wiki_id().unwrap(),
            group_id: SETTINGS.wiki.guest_group_id,
        })
        .await?;
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
    #[description = "Member of the organization"] mut member: Member,
    #[description = "Member name"] name: Option<String>,
    #[description = "Member Discord"] discord_member: Option<serenity::Member>,
    #[description = "Member Trello ID"] trello_id: Option<String>,
    #[description = "Member Trello Report Card ID"] trello_report_card_id: Option<String>,
    #[description = "Member role"] role: Option<MemberRole>,
    #[description = "Member wiki ID"] wiki_id: Option<i64>,
    #[description = "Refresh member's activity"] refresh_activity: Option<bool>,
) -> Result<(), Error> {
    let mut output = String::new();

    if let Some(new_name) = name {
        member.set_name(new_name)?
    }

    if let Some(new_discord_member) = discord_member {
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

        let dc_id = new_discord_member.user.id.get().to_string();

        member.set_discord_id(dc_id);
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

        // remove wiki account from old group
        member.unassign_wiki_group_by_role().await?;

        member.set_role(new_role);

        // assign wiki account to new group
        member.assign_wiki_group_by_role().await?;
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

    if let Some(refresh_activity) = refresh_activity {
        if refresh_activity {
            member.refresh_activity()?;
        }
    }

    if let Some(new_wiki_id) = wiki_id {
        // remove old account from group - only if it's not guest group
        let is_guest_group = member.wiki_group() != SETTINGS.wiki.guest_group_id;

        if is_guest_group {
            member.unassign_wiki_group_by_role().await?;

            // assign old account to guest group
            crate::wiki::assign_user_group(crate::wiki::assign_user_group::Variables {
                user_id: member.wiki_id().unwrap(),
                group_id: SETTINGS.wiki.guest_group_id,
            })
            .await?;

            member.set_wiki_id(new_wiki_id);

            // remove new account from guest group
            crate::wiki::unassign_user_group(crate::wiki::unassign_user_group::Variables {
                user_id: member.wiki_id().unwrap(),
                group_id: SETTINGS.wiki.guest_group_id,
            })
            .await?;

            // assign new account to group
            member.assign_wiki_group_by_role().await?;
        } else {
            member.set_wiki_id(new_wiki_id);
        }
    }

    member.update()?;

    info!("Member updated: {}", member);

    output.push_str(&format!("Updated {}", member));

    crate::discord::respond(ctx, output).await
}

#[poise::command(slash_command, rename = "list")]
pub async fn list_members(
    ctx: Context<'_>,
    #[description = "Page number"] page: Option<i64>,
    #[description = "Page size"] page_size: Option<i64>,
    #[description = "Member role"] role: Option<MemberRole>,
    #[description = "Member's activity"] activity: Option<Activity>,
) -> Result<(), Error> {
    let page = page.unwrap_or(1);

    let (members, total_pages) = Member::list(page, page_size, role, None, activity, None)?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}\n", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    crate::discord::respond(ctx, output).await
}
