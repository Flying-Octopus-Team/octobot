use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::framework::StandardFramework;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::interactions::application_command::ApplicationCommandOptionType;
use serenity::prelude::GatewayIntents;
use serenity::Client;

use crate::SETTINGS;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = GuildId(SETTINGS.server_id);

        let guild_command = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
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
                                    .kind(ApplicationCommandOptionType::User)
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
            })
        })
        .await
        .expect("Error creating global application command");

        println!("{:?}", guild_command);
    }
}

pub async fn start_bot() {
    let token = &SETTINGS.discord_token;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let framework = StandardFramework::new().configure(|c| c.prefix("~"));

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
