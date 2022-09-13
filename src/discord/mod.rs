use std::fmt::Write;

use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::framework::StandardFramework;
use serenity::model::application::interaction::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::voice::VoiceState;
use serenity::prelude::GatewayIntents;
use serenity::Client;
use tracing::{debug, error, info, warn};

use crate::SETTINGS;

struct Handler;

use crate::database::models::member::Member;
pub use crate::discord::commands::find_option_as_string;
pub use crate::discord::commands::find_option_value;
use crate::meeting::MeetingStatus;

mod commands;

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        debug!(?interaction, "Interaction created");
        if let Interaction::ApplicationCommand(command) = interaction {
            let content = match commands::handle_interaction_command(&ctx, &command).await {
                Ok(content) => content,
                Err(e) => format!("{:?}", e),
            };
            // separate content into chunks of 2000 characters
            // separate on newlines
            let mut content_chunks = content.lines();

            let mut output = String::new();

            for content_chunk in content_chunks.by_ref() {
                if output.len() + content_chunk.len() > 2000 {
                    match command
                        .create_interaction_response(&ctx.http, |response| {
                            response.interaction_response_data(|message| message.content(&content))
                        })
                        .await
                        .map_err(|e| format!("Error sending interaction response: {}", e))
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                    output.clear();
                }

                output.push_str(content_chunk);
                match writeln!(output) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("{}", e);
                    }
                }
            }

            match command
                .create_interaction_response(&ctx.http, |response| {
                    response.interaction_response_data(|message| message.content(&content))
                })
                .await
                .map_err(|e| format!("Error sending interaction response: {}", e))
            {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                }
            }
        };
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let read = ctx.data.read().await;
        let meeting_status = read.get::<MeetingStatus>().unwrap();
        let mut meeting_status = meeting_status.write().await;

        if meeting_status.is_meeting_ongoing()
            && old.is_none()
            && new.channel_id.is_some()
            && new.channel_id.unwrap() == SETTINGS.meeting.channel_id
        {
            match Member::find_by_discord_id(new.user_id.0.to_string()) {
                Ok(member) => {
                    let output = match meeting_status.add_member(&member) {
                        Ok(msg) => msg,
                        Err(e) => format!("{} could not join the meeting: {}", member.name(), e),
                    };
                    info!("{}", output);
                }
                Err(e) => warn!(
                    "User {} is not member of the organization: {:?}",
                    new.user_id.0, e
                ),
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = SETTINGS.server_id;

        let guild_command = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands::create_application_commands(commands)
        })
        .await
        .expect("Error creating global application command");

        debug!("{:?}", guild_command);

        let meeting_status = crate::meeting::create_meeting_job(&ctx).await.unwrap();

        ctx.data
            .write()
            .await
            .insert::<MeetingStatus>(meeting_status);
    }
}

pub async fn start_bot() {
    let token = &SETTINGS.discord_token;

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MEMBERS;

    let framework = StandardFramework::new().configure(|c| c.prefix("~"));

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    info!("Starting bot...");

    if let Err(why) = client.start().await {
        error!("An error occurred while running the client: {:?}", why);
    }
}
