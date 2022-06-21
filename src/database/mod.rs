use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use diesel_migrations::embed_migrations;
use lazy_static::lazy_static;

use crate::SETTINGS;

mod models;
mod schema;

pub type PgPool = Pool<ConnectionManager<PgConnection>>;

lazy_static! {
    static ref PG_POOL: PgPool = {
        let manager = ConnectionManager::new(&SETTINGS.database_url);
        Pool::new(manager).expect("Failed to create pool.")
    };
}

pub fn run_migrations() {
    embed_migrations!();

    let connection = PG_POOL.get().expect("Failed to get connection from pool.");

    embedded_migrations::run_with_output(&connection, &mut std::io::stdout())
        .expect("Failed to run migrations.");
}
