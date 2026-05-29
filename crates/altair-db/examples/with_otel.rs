//! Cross-crate auto-integration with `altair-otel`.
//!
//! `altair-db` enables `SeaORM`'s `sqlx_logging`, which emits `tracing` events
//! for every executed statement. Initialise `altair-otel` in the same process
//! and those events flow through the configured exporter.
//!
//! Run with: `cargo run --example with_otel -p altair-db`

use altair_db::prelude::*;
use altair_otel::{Config as OtelConfig, Exporter};
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("db-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    let backend = db.orm().get_database_backend();

    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE pings (n INTEGER)".to_string(),
        ))
        .await?;

    let pool = db.sqlite_pool().expect("sqlite pool");
    sqlx::query("INSERT INTO pings (n) VALUES (1)")
        .execute(pool)
        .await?;

    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pings")
        .fetch_one(pool)
        .await?;
    println!("rows: {n}");

    altair_otel::shutdown();
    Ok(())
}
