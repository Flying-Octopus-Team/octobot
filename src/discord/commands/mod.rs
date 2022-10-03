use serde_json::Value;
use serenity::model::application::command::CommandOptionType;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::application::interaction::application_command::CommandDataOption;
use serenity::{builder::CreateApplicationCommands, client::Context};
use tracing::warn;

mod meeting;
mod member;
mod report;
mod summary;

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
                _ => {
                    warn!("Unknown member option: {}", option.name);
                    Ok(String::from("Unknown subcommand"))
                }
            },
            None => {
                warn!("No member options");
                Ok(String::from("No subcommand specified"))
            }
        },
        "report" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "add" => report::add_report(ctx, command, option).await,
                "remove" => report::remove_report(option),
                "list" => report::list_reports(option),
                "update" => report::update_report(option),
                _ => {
                    warn!("Unknown report option: {}", option.name);
                    Ok(String::from("Unknown subcommand"))
                }
            },
            None => {
                warn!("No report options");
                Ok(String::from("No subcommand specified"))
            }
        },
        "summary" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "list" => summary::list_summaries(option),
                "resend" => summary::resend_summary(ctx, option).await,
                "preview" => summary::preview_summary(ctx, option).await,
                _ => {
                    warn!("Unknown summary option: {}", option.name);
                    Ok(String::from("Unknown subcommand"))
                }
            },
            None => {
                warn!("No summary options");
                Ok(String::from("No subcommand specified"))
            }
        },
        "meeting" => match command.data.options.first() {
            Some(option) => match option.name.as_str() {
                "end" => meeting::end_meeting(option, ctx).await,
                "status" => meeting::status_meeting(ctx).await,
                "list" => meeting::list_meetings(option).await,
                "plan" => meeting::plan_meeting(ctx, option).await,
                "set-note" => meeting::set_note(ctx, option).await,
                "add-member" => meeting::edit_meeting_members(ctx, option, false).await,
                "remove-member" => meeting::edit_meeting_members(ctx, option, true).await,
                _ => {
                    warn!("Unknown meeting option: {}", option.name);
                    Ok(String::from("Unknown subcommand"))
                }
            },
            None => {
                warn!("No meeting options");
                Ok(String::from("No subcommand specified"))
            }
        },
        _ => {
            warn!("Unknown command: {}", command.data.name);
            Ok(String::from("Unknown command"))
        }
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
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("discord-id")
                            .description("Add member by their Discord ID")
                            .required(true)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("trello-id")
                            .description("Add member by their Trello ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("trello-report-card-id")
                            .description("Add member by their Trello Report Card ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("name")
                            .description("Display name of the member")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("is-apprentice")
                            .description(
                                "Sets whether member is apprentice or not. Defaults to false",
                            )
                            .required(false)
                            .kind(CommandOptionType::Boolean)
                    })
            })
            .create_option(|option| {
                option
                    .name("remove")
                    .description("Remove member from the organization")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Remove member by their ID")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("list")
                    .description("List all members of the organization")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page")
                            .description("Page number")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page-size")
                            .description("Number of members per page")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
            })
            .create_option(|option| {
                option
                    .name("update")
                    .description("Update member's information")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Update member by their ID")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("name")
                            .description("Display name of the member")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("discord-id")
                            .description("Update member's Discord ID")
                            .required(false)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("trello-id")
                            .description("Update member's Trello ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("trello-report-card-id")
                            .description("Update member's Trello Report Card ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("is-apprentice")
                            .description("Sets whether member is apprentice or not")
                            .required(false)
                            .kind(CommandOptionType::Boolean)
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
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("content")
                            .description("Report content")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Add report for member")
                            .required(false)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("summary")
                            .description("Add report for summary")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("remove")
                    .description("Remove report")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Remove report by ID")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("list")
                    .description("List all reports")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page")
                            .description("Page number")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page-size")
                            .description("Number of reports per page")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("List reports for member")
                            .required(false)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("published")
                            .description("List (un)published reports")
                            .required(false)
                            .kind(CommandOptionType::Boolean)
                    })
            })
            .create_option(|option| {
                option
                    .name("update")
                    .description("Update report")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Update report by ID")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("content")
                            .description("Update report content")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Update report for member")
                            .required(false)
                            .kind(CommandOptionType::User)
                    })
            })
    });

    commands.create_application_command(|command| {
        command
            .name("summary")
            .description("Show weekly summary containing unpublished reports")
            .create_option(|option| {
                option
                    .name("list")
                    .description("List all summaries")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page")
                            .description("Page number")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page-size")
                            .description("Number of summaries per page")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
            })
            .create_option(|option| {
                option
                    .name("resend")
                    .description("Regenerate and resend summary to the channel")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Resend summary by ID")
                            .required(true)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("preview")
                    .description("Sends summary preview with generated member reports and note")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("id")
                            .description("Preview summary by ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("note")
                            .description("Summary note to be sent")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
    });

    commands.create_application_command(|command| {
        command
            .name("meeting")
            .description("Manage meetings")
            .create_option(|option| {
                option
                    .name("status")
                    .description("Show current meeting's status")
                    .kind(CommandOptionType::SubCommand)
            })
            .create_option(|option| {
                option
                    .name("end")
                    .description("End the current meeting")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("note")
                            .description("Note to add to the meeting")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("list")
                    .description("List all meetings")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page")
                            .description("Page number")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("page-size")
                            .description("Number of meetings per page")
                            .required(false)
                            .kind(CommandOptionType::Integer)
                    })
            })
            .create_option(|option| {
                option
                    .name("plan")
                    .description("Plan future meetings")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("schedule")
                            .description("Update next meetings schedule")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("channel")
                            .description("Update next meetings channel")
                            .required(false)
                            .kind(CommandOptionType::Channel)
                    })
            })
            .create_option(|option| {
                option
                    .name("set-note")
                    .description("Edit past/future meeting")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("note")
                            .description("Update past/future meeting summary note")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("meeting-id")
                            .description("Select past/future meeting by ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("add-member")
                    .description("Add member to the past/future meeting")
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Add member to the meeting")
                            .required(true)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("meeting-id")
                            .description("Add member to meeting by ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
            .create_option(|option| {
                option
                    .name("remove-member")
                    .description(
                        "Remove member from the current meeting. Default to current meeting",
                    )
                    .kind(CommandOptionType::SubCommand)
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("member")
                            .description("Remove member from the meeting")
                            .required(true)
                            .kind(CommandOptionType::User)
                    })
                    .create_sub_option(|sub_option| {
                        sub_option
                            .name("meeting-id")
                            .description("Remove member from meeting by ID")
                            .required(false)
                            .kind(CommandOptionType::String)
                    })
            })
    })
}

/// Find specified option's value by looking at the first option with the same name.
pub fn find_option_value<'a>(options: &'a [CommandDataOption], name: &str) -> Option<&'a Value> {
    options
        .iter()
        .find(|option| option.name.as_str() == name)
        .and_then(|option| option.value.as_ref())
}

pub fn find_option_as_string(options: &[CommandDataOption], name: &str) -> Option<String> {
    find_option_value(options, name).map(|value| value.as_str().unwrap().to_string())
}
