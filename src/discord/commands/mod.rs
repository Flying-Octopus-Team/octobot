use serde_json::Value;
use serenity::{
    builder::CreateApplicationCommands,
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
        ApplicationCommandOptionType,
    },
};

mod member;
mod report;

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
                "list" => member::list_members(option),
                _ => Ok(String::from("Unknown subcommand")),
            },
            None => Ok(String::from("No subcommand specified")),
        },
        "report" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "add" => report::add_report(ctx, command, option),
                "remove" => report::remove_report(option),
                "list" => report::list_reports(option),
                "update" => report::update_report(ctx, command, option),
                _ => Ok(String::from("Unknown subcommand")),
            },
            None => Ok(String::from("No subcommand specified")),
        },
        "summary" => {
            let publish = if let Some(option) = command.data.options.first() {
                option.value.as_ref().unwrap().as_bool().unwrap()
            } else {
                false
            };
            report::summary(ctx, command, publish).await
        },
        _ => Ok(String::from("Unknown command")),
    }
}

pub fn create_application_commands(
    commands: &mut CreateApplicationCommands,
) -> &mut CreateApplicationCommands {
    commands.create_application_command(|command| {
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
    });

    commands.create_application_command(|command| {
        command
            .name("report")
            .description("Manage member's reports")
            .create_option(|option| {
                option
                    .name("add")
                    .description("Add report")
                    .kind(ApplicationCommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("content")
                            .description("Report content")
                            .required(true)
                            .kind(ApplicationCommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Add report for member")
                            .required(false)
                            .kind(ApplicationCommandOptionType::User)
                    })
            })
            .create_option(|option| {
                option
                    .name("remove")
                    .description("Remove report")
                    .kind(ApplicationCommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Remove report by ID")
                            .required(true)
                            .kind(ApplicationCommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("list")
                    .description("List all reports")
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
                            .description("Number of reports per page")
                            .required(false)
                            .kind(ApplicationCommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("List reports for member")
                            .required(false)
                            .kind(ApplicationCommandOptionType::User)
                    })
            })
            .create_option(|option| {
                option
                    .name("update")
                    .description("Update report")
                    .kind(ApplicationCommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Update report by ID")
                            .required(true)
                            .kind(ApplicationCommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("content")
                            .description("Update report content")
                            .required(false)
                            .kind(ApplicationCommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Update report for member")
                            .required(false)
                            .kind(ApplicationCommandOptionType::User)
                    })
            })
    });

    commands.create_application_command(|command| {
        command
            .name("summary")
            .description("Show weekly summary containing unpublished reports")
            .create_option(|option| {
                option
                    .name("publish")
                    .description(
                        "Mark reports as published and do not display them in the next summary",
                    )
                    .kind(ApplicationCommandOptionType::Boolean)
                    .required(false)
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
