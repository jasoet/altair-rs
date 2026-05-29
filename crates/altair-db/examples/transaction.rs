//! Closure-style transaction across two writes.
//!
//! Run with: `cargo run --example transaction -p altair-db`

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
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, total INTEGER NOT NULL)".to_string(),
        ))
        .await?;

    db.transaction::<_, (), sea_orm::DbErr>(|tx| {
        Box::pin(async move {
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO users (name) VALUES ('alice')".to_string(),
            ))
            .await?;
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO orders (user_id, total) VALUES (1, 99)".to_string(),
            ))
            .await?;
            Ok(())
        })
    })
    .await?;

    println!("two-write transaction committed");
    Ok(())
}
