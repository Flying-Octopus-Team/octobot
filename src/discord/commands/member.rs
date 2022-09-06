use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use std::fmt::Write;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::discord::find_option_as_string;
use crate::{database::models::member::Member, SETTINGS};

use super::find_option_value;

pub async fn add_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding member");
    let mut member = Member::from(&option.options[..]);
    if member.discord_id().is_some() {
        let user_id = member.discord_id().unwrap().parse().unwrap();

        let dc_member = match ctx.cache.member(SETTINGS.server_id, user_id) {
            Some(dc_member) => dc_member,
            None => {
                let error_msg = format!("Member not found in the guild: {}", user_id);
                error!("{}", error_msg);
                return Err(error_msg.into());
            }
        };

        let new_name = if let Some(name) = find_option_as_string(&option.options[..], "name") {
            name
        } else {
            match dc_member.nick {
                Some(name) => name,
                None => dc_member.user.name,
            }
        };

        member.set_name(new_name);

        let guild_id = *command.guild_id.unwrap().as_u64();
        ctx.http
            .add_member_role(guild_id, user_id, SETTINGS.member_role_id, None)
            .await
            .unwrap();
    }

    let member = member.insert()?;

    info!("Member added: {:?}", member);

    Ok(format!("Added {}", member))
}

pub async fn remove_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
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

    let member = Member::find_by_id(Uuid::parse_str(&id)?)?;
    if member.discord_id().is_some() {
        let user_id = member.discord_id().unwrap().parse().unwrap();
        let guild_id = *command.guild_id.unwrap().as_u64();
        ctx.http
            .remove_member_role(guild_id, user_id, SETTINGS.member_role_id, None)
            .await
            .unwrap();
    }

    member.delete()?;

    info!("Member removed: {:?}", member);

    Ok(format!("Removed {}", member))
}

pub async fn update_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Updating member");
    let mut updated_member = Member::from(&option.options[..]);

    if let Some(name) = find_option_as_string(&option.options[..], "name") {
        updated_member.set_name(name);
    }

    let old_member = Member::find_by_id(updated_member.id())?;

    let updated_member = updated_member.update()?;

    if old_member.discord_id() != updated_member.discord_id() {
        if old_member.discord_id().is_some() {
            let user_id = old_member.discord_id().unwrap().parse().unwrap();
            let guild_id = *command.guild_id.unwrap().as_u64();
            ctx.http
                .remove_member_role(guild_id, user_id, SETTINGS.member_role_id, None)
                .await
                .unwrap();
        }
        if updated_member.discord_id().is_some() {
            let user_id = updated_member.discord_id().unwrap().parse().unwrap();
            let guild_id = *command.guild_id.unwrap().as_u64();
            ctx.http
                .add_member_role(guild_id, user_id, SETTINGS.member_role_id, None)
                .await
                .unwrap();
        }
    }

    info!("Member updated: {:?}", updated_member);

    Ok(format!("Updated {}", updated_member))
}

pub fn list_members(option: &CommandDataOption) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing members");
    let page = find_option_value(&option.options[..], "page").map_or(1, |x| x.as_i64().unwrap());
    let page_size =
        find_option_value(&option.options[..], "page_size").map(|v| v.as_i64().unwrap());

    let (members, total_pages) = Member::list(page, page_size)?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    Ok(output)
}
