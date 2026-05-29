//! SeaORM CRUD round-trip against SQLite.
//!
//! Run with: `cargo run --example basic_orm -p altair-db`

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;

    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_string(),
        ))
        .await?;

    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO users (name) VALUES ('alice'), ('bob')".to_string(),
        ))
        .await?;
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM users".to_string(),
        ))
        .await?
        .expect("count");
    let count: i64 = row.try_get("", "c")?;
    println!("users: {count}");

    Ok(())
}
