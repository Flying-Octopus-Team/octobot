use lazy_static::lazy_static;
use settings::Settings;

mod discord;
mod settings;

lazy_static! {
    static ref SETTINGS: Settings = settings::Settings::new().unwrap();
}

#[tokio::main]
async fn main() {
    discord::start_bot().await;
}
