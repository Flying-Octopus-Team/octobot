use std::fmt::Write;
use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::GatewayIntents;
use tokio::sync::RwLock;
use tracing::log::trace;
use tracing::{error, info, warn};

use crate::database::models::member::Member;
use crate::discord::commands::{meeting, member, report, summary};
use crate::meeting::MeetingStatus;
use crate::SETTINGS;

mod commands;

pub struct Data {
    pub meeting_status: Arc<RwLock<MeetingStatus>>,
}
pub type Error = anyhow::Error;
pub type Context<'a> = poise::Context<'a, Data, Error>;

async fn event_handler(
    ctx: &serenity::Context,
    event: &poise::Event<'_>,
    framework: poise::FrameworkContext<'_, Data, Error>,
) -> Result<(), Error> {
    match event {
        poise::Event::Ready { data_about_bot } => {
            info!("{} is connected!", data_about_bot.user.name);
            event_ready(ctx, framework).await;
        }
        poise::Event::VoiceStateUpdate { old, new } => {
            event_voice_state_update(framework, old, new).await;
        }
        _ => {}
    }

    Ok(())
}

async fn event_voice_state_update(
    framework: poise::FrameworkContext<'_, Data, Error>,
    old: &Option<serenity::VoiceState>,
    new: &serenity::VoiceState,
) {
    let mut meeting_status = framework.user_data.meeting_status.write().await;

    if meeting_status.is_meeting_ongoing()
        && old.is_none()
        && new.channel_id.is_some()
        && new.channel_id.unwrap() == meeting_status.channel().parse::<u64>().unwrap()
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

async fn event_ready(ctx: &serenity::Context, framework: poise::FrameworkContext<'_, Data, Error>) {
    // if the meeting is running when the bot starts, add all members to the meeting
    let mut meeting_status = framework.user_data.meeting_status.write().await;

    if meeting_status.is_meeting_ongoing() {
        let channel_id = SETTINGS.meeting.channel_id;
        let channel = channel_id.to_channel(&ctx).await.unwrap();

        for member in channel.guild().unwrap().members(&ctx).await.unwrap() {
            match Member::find_by_discord_id(member.user.id.0.to_string()) {
                Ok(member) => {
                    let output = match meeting_status.add_member(&member) {
                        Ok(msg) => msg,
                        Err(e) => {
                            format!("{} could not join the meeting: {}", member.name(), e)
                        }
                    };
                    info!("{}", output);
                }
                Err(e) => warn!(
                    "User {} is not member of the organization: {:?}",
                    member.user.id.0, e
                ),
            }
        }
    }
}

pub async fn start_bot() {
    let token = &SETTINGS.discord_token;

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MEMBERS;

    let options = poise::FrameworkOptions {
        commands: vec![member(), report(), summary(), meeting()],
        event_handler: |ctx, event, framework, _user_data| {
            Box::pin(event_handler(ctx, event, framework))
        },
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(String::from("~")),
            mention_as_prefix: true,
            edit_tracker: None,
            ..Default::default()
        },
        ..Default::default()
    };

    info!("Starting bot...");

    if let Err(why) = poise::Framework::builder()
        .token(token)
        .options(options)
        .intents(intents)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    SETTINGS.discord.server_id,
                )
                .await?;
                let meeting_status = crate::meeting::create_meeting_job(ctx).await.unwrap();
                Ok(Data { meeting_status })
            })
        })
        .run()
        .await
    {
        error!("An error occurred while running the client: {:?}", why);
    }
}

pub(crate) fn split_message(message: String) -> Result<Vec<String>, Error> {
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

async fn respond(ctx: Context<'_>, content: String) -> Result<(), Error> {
    let content_chunks = split_message(content)?;

    for content in content_chunks {
        poise::reply::send_reply(ctx, |m| {
            m.content(content).allowed_mentions(|m| m.empty_parse())
        })
        .await?;
    }

    Ok(())
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
