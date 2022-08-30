use config::Config;
use config::ConfigError;
use config::File;
use serde::Deserialize;
use serenity::model::prelude::ChannelId;
use tracing::info;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub discord_token: String,
    pub database_url: String,
    pub member_role_id: u64,
    pub server_id: u64,
    pub meeting: Meeting,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meeting {
    pub channel_id: ChannelId,
    pub cron: String,
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
