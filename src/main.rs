#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate diesel;

use lazy_static::lazy_static;
use settings::Settings;

mod database;
mod discord;
mod settings;

lazy_static! {
    static ref SETTINGS: Settings = settings::Settings::new().unwrap();
}

#[tokio::main]
async fn main() {
    database::run_migrations();
    discord::start_bot().await;
}
