//! Postgres integration via testcontainers.
//!
//! Skipped on non-Linux because Docker availability is unreliable on
//! macOS/Windows CI runners.

#![cfg(all(feature = "postgres", target_os = "linux"))]

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn start() -> (Db, testcontainers::ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.expect("start postgres");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let db = Db::connect(Config::from_url(url)).await.expect("connect");
    (db, container)
}

#[tokio::test]
async fn connects_and_reports_postgres_backend() {
    let (db, _c) = start().await;
    assert_eq!(db.backend(), Backend::Postgres);
    db.ping().await.unwrap();
}

#[tokio::test]
async fn materialized_view_round_trip_via_raw_sqlx() {
    let (db, _c) = start().await;
    let pool = db.pg_pool().expect("pg pool");

    sqlx::query("CREATE TABLE orders (id SERIAL PRIMARY KEY, amount NUMERIC NOT NULL)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO orders (amount) VALUES (10), (20), (30)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE MATERIALIZED VIEW order_totals AS \
         SELECT COUNT(*) AS n, SUM(amount)::text AS total FROM orders",
    )
    .execute(pool)
    .await
    .unwrap();

    let (n, total): (i64, String) = sqlx::query_as("SELECT n, total FROM order_totals")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 3);
    assert_eq!(total, "60");
}

#[tokio::test]
async fn orm_crud_round_trip() {
    let (db, _c) = start().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE widgets (id SERIAL PRIMARY KEY, name TEXT NOT NULL)".to_string(),
        ))
        .await
        .unwrap();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO widgets (name) VALUES ('a'), ('b')".to_string(),
        ))
        .await
        .unwrap();
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM widgets".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.try_get("", "c").unwrap();
    assert_eq!(count, 2);
}
