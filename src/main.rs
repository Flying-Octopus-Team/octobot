#[macro_use]
extern crate diesel;

use lazy_static::lazy_static;
use settings::Settings;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt::Layer,
    prelude::__tracing_subscriber_SubscriberExt,
};

mod database;
mod discord;
pub mod error;
mod meeting;
mod settings;
pub mod wiki;

lazy_static! {
    static ref SETTINGS: Settings = settings::Settings::new().expect("Unable to load settings");
}

#[tokio::main]
async fn main() {
    let _guard = setup_tracing();

    database::run_migrations();
    discord::start_bot().await;
}

fn setup_tracing() -> WorkerGuard {
    let dir = std::env::current_dir()
        .expect("Unable to get current directory")
        .join("logs");

    let file_appender = tracing_appender::rolling::daily(dir, "octobot.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let mut file_layer = Layer::new().with_writer(non_blocking);
    file_layer.set_ansi(false);

    let std_layer = Layer::new().pretty().with_writer(std::io::stdout);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .with(file_layer)
        .with(std_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set a global subscriber");

    guard
}
