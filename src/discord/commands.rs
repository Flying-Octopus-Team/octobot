use serenity::{
    builder::CreateApplicationCommand,
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
        ApplicationCommandOptionType,
    },
};
use uuid::Uuid;

use crate::{database::models::member::Member, SETTINGS};

pub async fn handle_interaction_command<'a>(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<String, Box<dyn std::error::Error>> {
    match command.data.name.as_str() {
        "member" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "add" => add_member(ctx, command, option).await,
                "remove" => remove_member(ctx, command, option).await,
                "update" => update_member(ctx, command, option).await,
                "list" => {
                    todo!()
                }
                _ => {
                    //"Unknown command".to_string()
                    todo!()
                }
            },
            None => {
                //"Unknown command".to_string()
                todo!()
            }
        },
        _ => {
            //"Unknown command".to_string()
            todo!()
        }
    }
}

async fn add_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let member = Member::from(&option.options[..]);
    let member = member.insert()?;
    if member.discord_id().is_some() {
        let user_id = member.discord_id().unwrap().parse().unwrap();
        let guild_id = *command.guild_id.unwrap().as_u64();
        ctx.http
            .add_member_role(guild_id, user_id, SETTINGS.member_role_id, None)
            .await
            .unwrap();
    }

    Ok(format!("Added {}", member))
}

async fn remove_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
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

    Ok(format!("Removed {}", member))
}

async fn update_member(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    option: &ApplicationCommandInteractionDataOption,
) -> Result<String, Box<dyn std::error::Error>> {
    let updated_member = Member::from(&option.options[..]);

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

    Ok(format!("Updated {}", updated_member))
}

pub fn create_application_commands(
    command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
    command
        .name("member")
        .description("Manage organization's members")
        .create_option(|option| {
            option
                .name("add")
                .description("Add member to the organization")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("discord_id")
                        .description("Add member by their Discord ID")
                        .required(true)
                        .kind(ApplicationCommandOptionType::User)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("trello_id")
                        .description("Add member by their Trello ID")
                        .required(false)
                        .kind(ApplicationCommandOptionType::String)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("trello_report_card_id")
                        .description("Add member by their Trello Report Card ID")
                        .required(false)
                        .kind(ApplicationCommandOptionType::String)
                })
        })
        .create_option(|option| {
            option
                .name("remove")
                .description("Remove member from the organization")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("id")
                        .description("Remove member by their ID")
                        .required(true)
                        .kind(ApplicationCommandOptionType::String)
                })
        })
        .create_option(|option| {
            option
                .name("list")
                .description("List all members of the organization")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("page")
                        .description("List all members of the organization")
                        .required(false)
                        .kind(ApplicationCommandOptionType::Integer)
                })
        })
        .create_option(|option| {
            option
                .name("update")
                .description("Update member's information")
                .kind(ApplicationCommandOptionType::SubCommand)
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("id")
                        .description("Update member by their ID")
                        .required(true)
                        .kind(ApplicationCommandOptionType::String)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("discord_id")
                        .description("Update member's Discord ID")
                        .required(false)
                        .kind(ApplicationCommandOptionType::User)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("trello_id")
                        .description("Update member's Trello ID")
                        .required(false)
                        .kind(ApplicationCommandOptionType::String)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("trello_report_card_id")
                        .description("Update member's Trello Report Card ID")
                        .required(false)
                        .kind(ApplicationCommandOptionType::String)
                })
        })
}
