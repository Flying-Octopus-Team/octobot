use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lazy_static::lazy_static;
use tracing::info;

use crate::SETTINGS;

pub mod models;
pub mod pagination;
mod schema;

pub type PgPool = Pool<ConnectionManager<PgConnection>>;

lazy_static! {
    static ref PG_POOL: PgPool = {
        let manager = ConnectionManager::<PgConnection>::new(&SETTINGS.database_url);
        info!("Connecting to database");
        Pool::new(manager).expect("Failed to create pool.")
    };
}

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_migrations() {
    let mut conn = PG_POOL.get().expect("Failed to get connection from pool.");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations.");
    info!("Migrations run successfully.");
}
