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
use tracing::log::trace;
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
            let mut content_chunks = match split_message(content) {
                Ok(content_chunks) => content_chunks.into_iter(),
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            match command
                .create_interaction_response(&ctx.http, |response| {
                    response.interaction_response_data(|message| {
                        message.content(content_chunks.next().unwrap())
                    })
                })
                .await
                .map_err(|e| format!("Error sending interaction response: {}", e))
            {
                Ok(_) => {
                    for content in content_chunks {
                        match command
                            .create_followup_message(&ctx.http, |message| message.content(content))
                            .await
                            .map_err(|e| format!("Error sending followup message: {}", e))
                        {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                }
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

        // insert new meeting only when, there's no another one
        if ctx.data.read().await.get::<MeetingStatus>().is_none() {
            let meeting_status = crate::meeting::create_meeting_job(&ctx).await.unwrap();

            ctx.data
                .write()
                .await
                .insert::<MeetingStatus>(meeting_status);
        }
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

pub(crate) fn split_message(message: String) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut message_chunks = message.lines();
    let mut output = String::new();
    let mut messages = Vec::new();
    for message_chunk in message_chunks.by_ref() {
        if output.len() + message_chunk.len() > 2000 {
            output = output.trim_end().to_string();
            messages.push(output.clone());
            trace!("Adding chunk to messages: {}", output);
            output.clear();
        }
        writeln!(output, "{}", message_chunk)?;
    }
    output = output.trim_end().to_string();
    messages.push(output);
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use crate::discord::split_message;

    #[test]
    fn test_split_message() {
        // string with over 2000 characters

        let first_part = String::from("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Non diam phasellus vestibulum lorem sed. Velit euismod in pellentesque massa placerat. Tellus id interdum velit laoreet id. Sollicitudin ac orci phasellus egestas tellus rutrum tellus. Tempor id eu nisl nunc mi ipsum faucibus vitae aliquet. Turpis egestas integer eget aliquet nibh praesent. Enim lobortis scelerisque fermentum dui faucibus in. Pellentesque diam volutpat commodo sed egestas egestas fringilla phasellus faucibus. Sed blandit libero volutpat sed. Sollicitudin aliquam ultrices sagittis orci. Massa tempor nec feugiat nisl pretium fusce. Duis ut diam quam nulla porttitor massa id. Nibh ipsum consequat nisl vel pretium. Varius sit amet mattis vulputate enim nulla aliquet. Vestibulum sed arcu non odio euismod lacinia at quis. Sed vulputate mi sit amet. Elementum facilisis leo vel fringilla est ullamcorper eget.\n
        In fermentum et sollicitudin ac orci phasellus egestas tellus. Est ante in nibh mauris cursus mattis molestie a. Vitae ultricies leo integer malesuada nunc vel risus commodo. In ornare quam viverra orci sagittis eu. Vulputate odio ut enim blandit volutpat maecenas volutpat blandit. Arcu risus quis varius quam quisque id diam vel. Id nibh tortor id aliquet lectus proin nibh nisl. Condimentum vitae sapien pellentesque habitant morbi tristique senectus et. Id diam maecenas ultricies mi eget mauris pharetra. Interdum varius sit amet mattis. Semper feugiat nibh sed pulvinar. Cras adipiscing enim eu turpis egestas pretium aenean pharetra. Condimentum lacinia quis vel eros donec ac odio tempor. Donec massa sapien faucibus et molestie. Aenean et tortor at risus viverra adipiscing at in tellus.");

        let second_part = String::from("Duis convallis convallis tellus id interdum. Aliquet risus feugiat in ante. Tellus orci ac auctor augue. Nisi quis eleifend quam adipiscing vitae proin sagittis. Sed odio morbi quis commodo. Egestas purus viverra accumsan in nisl nisi scelerisque eu. Diam sollicitudin tempor id eu nisl nunc. Egestas maecenas pharetra convallis posuere morbi leo. Auctor augue mauris augue neque. Nullam non nisi est sit amet facilisis. Donec ultrices tincidunt arcu non sodales neque sodales.");

        let message = format!("{}\n{}", first_part, second_part);

        let messages = split_message(message).unwrap();

        assert_eq!(messages.len(), 2);

        assert_eq!(messages[0], first_part);

        assert_eq!(messages[1], second_part);
    }
}
