//! Postgres-only example: materialised view via raw sqlx.
//!
//! Run with a Postgres URL on stdin:
//!
//!   DATABASE_URL=postgres://user:pass@host/db \
//!     cargo run --example materialized_view -p altair-db --features postgres

use altair_db::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());
    let db = Db::connect(Config::from_url(url)).await?;
    assert_eq!(db.backend(), Backend::Postgres);

    let pool = db.pg_pool().expect("pg pool");

    sqlx::query("DROP MATERIALIZED VIEW IF EXISTS order_totals")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DROP TABLE IF EXISTS orders")
        .execute(pool)
        .await
        .ok();
    sqlx::query("CREATE TABLE orders (id SERIAL PRIMARY KEY, amount NUMERIC NOT NULL)")
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO orders (amount) VALUES (10), (20), (30)")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE MATERIALIZED VIEW order_totals AS \
         SELECT COUNT(*) AS n, SUM(amount) AS total FROM orders",
    )
    .execute(pool)
    .await?;
    sqlx::query("REFRESH MATERIALIZED VIEW order_totals")
        .execute(pool)
        .await?;

    let (n, total): (i64, String) =
        sqlx::query_as("SELECT n, total::text FROM order_totals")
            .fetch_one(pool)
            .await?;
    println!("count={n} total={total}");
    Ok(())
}
