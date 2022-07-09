use serenity::async_trait;

use serenity::client::{Context, EventHandler};
use serenity::framework::StandardFramework;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;

use serenity::model::interactions::Interaction;
use serenity::prelude::GatewayIntents;
use serenity::Client;

use crate::SETTINGS;

struct Handler;

pub use crate::discord::commands::find_option_as_string;
pub use crate::discord::commands::find_option_value;

mod commands;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let content = match commands::handle_interaction_command(&ctx, &command).await {
                Ok(content) => content,
                Err(e) => format!("{:?}", e),
            };
            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response.interaction_response_data(|message| message.content(content))
                })
                .await
            {
                println!("Error creating interaction response: {:?}", why);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let guild_id = GuildId(SETTINGS.server_id);

        let guild_command = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands.create_application_command(commands::create_application_commands)
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
