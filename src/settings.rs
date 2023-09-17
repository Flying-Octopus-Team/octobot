use config::{Config, ConfigError, File};
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use serenity::model::prelude::{ChannelId, GuildId, RoleId};
use tracing::info;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub database_url: String,
    pub meeting: Meeting,
    pub discord: Discord,
    pub wiki: Wiki,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meeting {
    pub channel_id: ChannelId,
    pub cron: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Discord {
    pub token: String,
    pub member_role: RoleId,
    pub apprentice_role: RoleId,
    pub summary_channel: ChannelId,
    pub server_id: GuildId,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Wiki {
    pub url: String,
    pub graphql: String,
    pub token: String,
    pub provider_key: String,
    pub member_group_id: i64,
    pub guest_group_id: i64,
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
