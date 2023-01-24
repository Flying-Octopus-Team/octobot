use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use std::fmt::Write;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::database::models::member::MemberRole;
use crate::discord::find_option_as_string;
use crate::{database::models::member::Member, SETTINGS};

use super::find_option_value;

pub async fn add_member(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding member");
    let member = Member::from(&option.options[..]);
    let mut new_name = String::new();
    if let Some(discord_id) = member.discord_id() {
        let discord_id = discord_id.parse().unwrap();

        let dc_member = match ctx.cache.member(SETTINGS.discord.server_id, discord_id) {
            Some(dc_member) => dc_member,
            None => {
                let error_msg = format!("Member not found in the guild: {}", discord_id);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        };

        new_name = if let Some(name) = find_option_as_string(&option.options[..], "name") {
            name
        } else {
            match dc_member.nick {
                Some(name) => name,
                None => dc_member.user.name,
            }
        };

        member.role().add_role(ctx, discord_id).await?;
    }

    // check if member is already in the database
    if let Ok(member) = Member::find_by_discord_id(member.discord_id().unwrap()) {
        let mut msg = String::new();
        write!(
            msg,
            "Member with this Discord ID already exists in the database with the following information:
            Name: {}
            Discord ID: {}
            UUID: {}
            Role: {}",
            member.name(),
            member.discord_id().unwrap(),
            member.id(),
            member.role()
        )?;
        return Ok(msg);
    }

    let mut member = match member.insert() {
        Ok(member) => member,
        Err(e) => {
            let error_msg = format!("Failed to insert member into database: {}", e);
            error!("{}", error_msg);
            return Err(error_msg.into());
        }
    };

    match member.set_name(new_name) {
        Ok(_) => {}
        Err(why) => {
            let error_msg = format!("Failed to update member name: {}", why);
            error!("{}", error_msg);
            return Err(error_msg.into());
        }
    }

    info!("Member added: {:?}", member);

    Ok(format!("Added {}", member))
}

pub async fn remove_member(
    ctx: &Context,
    _command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Removing member");
    let id = option.options[0]
        .value
        .as_ref()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let mut output = String::new();

    let member = Member::find_by_id(Uuid::parse_str(&id)?)?;
    if let Some(user_id) = member.discord_id() {
        let user_id = user_id.parse().unwrap();

        match member.role().remove_role(ctx, user_id).await {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to remove member role: {}", why);
                error!("{}", error_msg);
                output.push_str(&error_msg);
                output.push('\n');
            }
        }
    }

    let hard_delete = find_option_value(&option.options[..], "hard_delete")
        .map(|v| v.as_bool().unwrap())
        .unwrap_or(false);

    if hard_delete {
        member.hard_delete()?;
    } else {
        member.delete()?;
    }

    info!("Member removed: {:?}", member);

    output.push_str(&format!("Removed {}", member));

    Ok(output)
}

pub async fn update_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Updating member");
    let mut updated_member = Member::from(&option.options[..]);

    let old_member = Member::find_by_id(updated_member.id())?;

    if let Some(name) = find_option_as_string(&option.options[..], "name") {
        match updated_member.set_name(name) {
            Ok(_) => {}
            Err(why) => {
                let error_msg = format!("Failed to update member name: {}", why);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        }
    } else {
        updated_member.set_name(old_member.name())?;
    }

    let updated_member = updated_member.update()?;

    if old_member.discord_id() != updated_member.discord_id() && old_member.discord_id().is_some() {
        let user_id = old_member.discord_id().unwrap().parse().unwrap();
        let guild_id = *command.guild_id.unwrap().as_u64();
        ctx.http
            .remove_member_role(guild_id, user_id, SETTINGS.discord.member_role.0, None)
            .await
            .unwrap();
        ctx.http
            .remove_member_role(guild_id, user_id, SETTINGS.discord.apprentice_role.0, None)
            .await
            .unwrap();
    }
    if let Some(user_id) = updated_member.discord_id() {
        let user_id = user_id.parse().unwrap();

        MemberRole::swap_roles(updated_member.role(), old_member.role(), ctx, user_id).await?;
    }

    info!("Member updated: {:?}", updated_member);

    Ok(format!("Updated {}", updated_member))
}

pub fn list_members(option: &CommandDataOption) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing members");
    let page = find_option_value(&option.options[..], "page").map_or(1, |x| x.as_i64().unwrap());
    let page_size =
        find_option_value(&option.options[..], "page-size").map(|v| v.as_i64().unwrap());

    let (members, total_pages) = Member::list(page, page_size)?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    Ok(output)
}
