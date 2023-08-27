use std::fmt::Write;

use poise::serenity_prelude as serenity;
use poise::serenity_prelude::CreateButton;
use poise::serenity_prelude::CreateMessage;
use tracing::error;
use tracing::info;

use super::Context;
use super::Error;
use crate::database::models::member::Member;
use crate::database::models::member::MemberRole;
use crate::SETTINGS;

/// Modal to get the Discord email of a user to connect or create a new wiki account
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
    let mut button = CreateButton::default();
    button
        .label("Discord email")
        .custom_id("discord_email_button");
    button
}

fn dm_wiki_details<'a, 'b>(c: &'a mut CreateMessage<'b>) -> &'a mut CreateMessage<'b> {
    c.content("Welcome to Flying Octopus! In order to create your account on our wiki, please provide your Discord email address (the one you use to log into Discord).")
        .components(|c|
            c.create_action_row(|a|
                a.add_button(discord_email_button())
            )
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
    info!("Adding member");

    let mut output = String::new();

    let name = if let Some(name) = name {
        name
    } else {
        member.display_name().to_string()
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

    let mut member = match member.insert() {
        Ok(member) => member,
        Err(e) => {
            let error_msg = format!("Failed to insert member into database: {}", e);
            error!("{}", error_msg);
            return Err(anyhow!(error_msg));
        }
    };

    info!("Member added: {:?}", member);

    output.push_str(&format!("Added {}", member));

    let mut ask_wiki_details = false;

    if member.wiki_id().is_none() {
        output.push_str("\nInstructions to create wiki account sent via DM to the new member.");
        ask_wiki_details = true;
    }

    crate::discord::respond(ctx, output).await?;

    if ask_wiki_details {
        // DM new member to get either their Discord email to create new wiki account or their wiki ID
        let dm = discord_id.create_dm_channel(&ctx).await?;

        let msg = dm.send_message(&ctx, dm_wiki_details).await?;

        let mut wiki_id = None;

        while let Some(interaction) =
            poise::serenity_prelude::CollectComponentInteraction::new(ctx.serenity_context())
                .channel_id(dm.id)
                .author_id(discord_id)
                .message_id(msg.id)
                .await
        {
            info!("User {} responded to wiki email modal", interaction.user.id);

            let email = poise::execute_modal_on_component_interaction::<WikiEmailModal>(
                ctx,
                interaction,
                None,
                None,
            )
            .await?;

            if let Some(email) = email {
                let email = email.wiki_email;

                let user_id = crate::wiki::find_or_create_user(email, member.name().clone())
                    .await
                    .unwrap();

                wiki_id = Some(user_id);

                break;
            }
        }

        if let Some(wiki_id) = wiki_id {
            member.set_wiki_id(wiki_id);

            member
                .unassign_wiki_group(SETTINGS.wiki.guest_group_id)
                .await?;

            member.assign_wiki_group_by_role().await?;

            match member.update() {
                Ok(_) => {}
                Err(e) => {
                    let error_msg = format!("Failed to update member in database: {}", e);
                    error!("{}", error_msg);
                    return Err(anyhow!(error_msg));
                }
            }
        }

        let _ = dm
            .send_message(&ctx, |m| {
                m.content(format!(
                    "Your wiki account has been created. You can now login at {}",
                    SETTINGS.wiki.url
                ))
            })
            .await;
    }

    Ok(())
}

#[poise::command(slash_command, rename = "remove")]
pub async fn remove_member(
    ctx: Context<'_>,
    #[description = "Member of the organization"] member: Member,
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
