use config::Config;
use config::ConfigError;
use config::File;
use serde::Deserialize;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::GuildId;
use serenity::model::prelude::RoleId;
use tracing::info;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub discord_token: String,
    pub database_url: String,
    pub meeting: Meeting,
    pub discord: Discord,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meeting {
    pub channel_id: ChannelId,
    pub cron: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Discord {
    pub member_role: RoleId,
    pub apprentice_role: RoleId,
    pub summary_channel: ChannelId,
    pub server_id: GuildId,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        info!("Creating settings");

        let s = Config::builder()
            .add_source(File::with_name("config/config"))
            .build()?;

        s.try_deserialize()
    }
}
