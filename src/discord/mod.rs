use serenity::async_trait;
use serenity::client::{Context, EventHandler};
use serenity::framework::StandardFramework;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::model::application::interaction::Interaction;
use serenity::model::voice::VoiceState;
use serenity::prelude::GatewayIntents;
use serenity::Client;
use tracing::{debug, error, info};

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
            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response.interaction_response_data(|message| message.content(content))
                })
                .await
            {
                error!("Error creating interaction response: {:?}", why);
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let read = ctx.data.read().await;
        let meeting_status = read.get::<MeetingStatus>().unwrap();
        let mut meeting_status = meeting_status.write().await;

        if meeting_status.is_meeting_ongoing()
            && new.channel_id.is_some()
            && new.channel_id.unwrap() == SETTINGS.meeting.channel_id
        {
            match Member::find_by_discord_id(new.user_id.0.to_string()) {
                Ok(member) => {
                    meeting_status.add_member(member.id()).unwrap();
                }
                Err(e) => println!("User is not member of the organization: {:?}", e),
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = GuildId(SETTINGS.server_id);

        let guild_command = GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
            commands::create_application_commands(commands)
        })
        .await
        .expect("Error creating global application command");

        debug!("{:?}", guild_command);

        let meeting_status = crate::meeting::create_meeting_job(ctx.cache.clone())
            .await
            .unwrap();

        ctx.data
            .write()
            .await
            .insert::<MeetingStatus>(meeting_status);
    }
}

pub async fn start_bot() {
    let token = &SETTINGS.discord_token;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILDS
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
