use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use std::fmt::Write;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::database::models::member::Member as DbMember;
use crate::discord::find_option_as_string;
use crate::framework::member::Member;
use crate::framework::member::MemberBuilder;
use crate::SETTINGS;

use super::find_option_value;

pub async fn add_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Adding member");
    let member = DbMember::from(&option.options[..]);
    let mut new_name = String::new();
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

        new_name = if let Some(name) = find_option_as_string(&option.options[..], "name") {
            name
        } else {
            match dc_member.nick {
                Some(name) => name,
                None => dc_member.user.name,
            }
        };

        let guild_id = *command.guild_id.unwrap().as_u64();
        if member.is_apprentice() {
            ctx.http
                .add_member_role(guild_id, user_id, SETTINGS.apprentice_role_id.0, None)
                .await?;
        } else {
            ctx.http
                .add_member_role(guild_id, user_id, SETTINGS.member_role_id.0, None)
                .await?;
        }
    }

    // check if member is already in the database
    if let Ok(member) = DbMember::find_by_discord_id(member.discord_id().unwrap()) {
        let mut msg = String::new();
        write!(
            msg,
            "Member with this Discord ID already exists in the database with the following information:
            Name: {}
            Discord ID: {}
            UUID: {}
            Apprentice: {}",
            member.name(),
            member.discord_id().unwrap(),
            member.id(),
            member.is_apprentice()
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

    let member = DbMember::find_by_id(Uuid::parse_str(&id)?)?;
    if member.discord_id().is_some() {
        let user_id = member.discord_id().unwrap().parse().unwrap();
        let guild_id = *command.guild_id.unwrap().as_u64();
        ctx.http
            .remove_member_role(guild_id, user_id, SETTINGS.member_role_id.0, None)
            .await
            .unwrap();
        ctx.http
            .remove_member_role(guild_id, user_id, SETTINGS.apprentice_role_id.0, None)
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
    let updated_member = MemberBuilder::from(option);

    let id = Uuid::parse_str(find_option_value(&option.options[..], "id").unwrap().as_str().unwrap())?;

    let mut old_member = Member::get(id, ctx).await?;

    old_member.edit(updated_member, ctx).await.unwrap();

    old_member = old_member.update().unwrap();

    info!("Member updated: {:?}", old_member);

    Ok(format!("Updated {}", old_member))
}

pub async fn list_members(
    ctx: &Context,
    option: &CommandDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("Listing members");
    let page = find_option_value(&option.options[..], "page").map_or(1, |x| x.as_i64().unwrap());
    let page_size =
        find_option_value(&option.options[..], "page-size").map(|v| v.as_i64().unwrap());

    let member_filter = MemberBuilder::from(option);

    let (members, total_pages) = Member::list(member_filter, ctx, page, page_size).await?;

    let mut output = String::new();

    for member in members {
        writeln!(&mut output, "{}\n", member)?;
    }
    write!(&mut output, "Page: {page}/{total_pages}")?;

    Ok(output)
}
