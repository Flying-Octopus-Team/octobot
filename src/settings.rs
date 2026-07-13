use config::{Config, ConfigError, File};
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use serenity::model::prelude::{ChannelId, GuildId, RoleId};
use tracing::info;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub database_url: String,
    pub activity_threshold_days: i64,
    /// Initial state of silent mode. When enabled, the bot does not act on
    /// its own (e.g. does not start scheduled meetings); it only responds to
    /// explicit commands. Defaults to `true` when missing from the config.
    /// Admins can toggle it at runtime with the `/silent-mode` command.
    #[serde(default = "default_silent_mode")]
    pub silent_mode: bool,
    /// Presence gate for scheduled meetings. When enabled, a scheduled
    /// meeting will not start unless at least one human (non-bot) member is
    /// already connected to the meeting's voice channel. This is an
    /// additional, independent safety check on top of `silent_mode`: it
    /// applies even when silent mode is disabled. Defaults to `true` when
    /// missing from the config.
    #[serde(default = "default_require_presence")]
    pub require_presence: bool,
    pub meeting: Meeting,
    pub discord: Discord,
    pub wiki: Wiki,
}

fn default_silent_mode() -> bool {
    true
}

fn default_require_presence() -> bool {
    true
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
