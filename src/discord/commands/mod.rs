use serde_json::Value;
use serenity::{
    builder::CreateApplicationCommand,
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
        ApplicationCommandOptionType,
    },
};

mod member;

pub async fn handle_interaction_command<'a>(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<String, Box<dyn std::error::Error>> {
    match command.data.name.as_str() {
        "member" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "add" => member::add_member(ctx, command, option).await,
                "remove" => member::remove_member(ctx, command, option).await,
                "update" => member::update_member(ctx, command, option).await,
                "list" => member::list_members(option).await,
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
                        .description("Page number")
                        .required(false)
                        .kind(ApplicationCommandOptionType::Integer)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("page_size")
                        .description("Number of members per page")
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

pub fn find_option_value<'a>(
    options: &'a [ApplicationCommandInteractionDataOption],
    name: &str,
) -> Option<&'a Value> {
    options
        .iter()
        .find(|option| option.name.as_str() == name)
        .and_then(|option| option.value.as_ref())
}

pub fn find_option_as_string(
    options: &[ApplicationCommandInteractionDataOption],
    name: &str,
) -> Option<String> {
    find_option_value(options, name).map(|value| value.as_str().unwrap().to_string())
}
