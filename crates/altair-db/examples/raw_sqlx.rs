//! Reach for raw sqlx via the pool for power-user queries.
//!
//! Run with: `cargo run --example raw_sqlx -p altair-db`

use altair_db::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    let pool = db.sqlite_pool().expect("sqlite pool");

    sqlx::query("CREATE TABLE events (ts INTEGER NOT NULL, kind TEXT NOT NULL)")
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO events (ts, kind) VALUES (?1, ?2), (?3, ?4)")
        .bind(1)
        .bind("login")
        .bind(2)
        .bind("logout")
        .execute(pool)
        .await?;

    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT ts, kind FROM events ORDER BY ts")
        .fetch_all(pool)
        .await?;
    for (ts, kind) in rows {
        println!("ts={ts} kind={kind}");
    }
    Ok(())
}
