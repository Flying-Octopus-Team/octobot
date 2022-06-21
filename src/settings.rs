use config::Config;
use config::ConfigError;
use config::File;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub discord_token: String,
    pub database_url: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
            .add_source(File::with_name("config/config"))
            .build()?;

        s.try_deserialize()
    }
}
