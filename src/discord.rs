use serenity::async_trait;
use serenity::client::Context;
use serenity::client::EventHandler;
use serenity::model::gateway::Ready;
use serenity::prelude::GatewayIntents;
use serenity::Client;

use crate::SETTINGS;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

pub async fn start_bot() {
    let token = &SETTINGS.discord_token;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
