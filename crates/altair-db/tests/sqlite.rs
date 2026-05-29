//! Integration tests that exercise the full `Db` surface over `SQLite`.
//!
//! Always runs in CI (no infrastructure required).

#![cfg(feature = "sqlite")]

use std::io::Write;

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

async fn connect() -> Db {
    Db::connect(Config::from_url("sqlite::memory:"))
        .await
        .expect("sqlite::memory: should connect")
}

#[tokio::test]
async fn connects_and_reports_backend() {
    let db = connect().await;
    assert_eq!(db.backend(), Backend::Sqlite);
}

#[tokio::test]
async fn ping_succeeds() {
    let db = connect().await;
    db.ping().await.unwrap();
}

#[tokio::test]
async fn migrate_runs_and_tracks_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = std::fs::File::create(dir.path().join("20260101000000_init.sql")).unwrap();
    writeln!(
        f,
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL);"
    )
    .unwrap();
    let mut g = std::fs::File::create(dir.path().join("20260102000000_add_seen.sql")).unwrap();
    writeln!(g, "ALTER TABLE notes ADD COLUMN seen BOOLEAN DEFAULT 0;").unwrap();
    drop((f, g));

    let db = connect().await;
    db.migrate(dir.path()).await.unwrap();

    let stmt = Statement::from_string(
        db.orm().get_database_backend(),
        "SELECT version FROM _sqlx_migrations ORDER BY version".to_string(),
    );
    let rows = db.orm().query_all(stmt).await.unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn raw_sqlx_query_via_pool() {
    let db = connect().await;
    let pool = db.sqlite_pool().expect("sqlite pool");
    let (one,): (i64,) = sqlx::query_as("SELECT 1").fetch_one(pool).await.unwrap();
    assert_eq!(one, 1);
}

#[tokio::test]
async fn transaction_commits_and_rolls_back() {
    let db = connect().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE t (n INTEGER)".to_string(),
        ))
        .await
        .unwrap();

    // commit
    db.transaction::<_, (), sea_orm::DbErr>(|tx| {
        Box::pin(async move {
            tx.execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "INSERT INTO t (n) VALUES (1)".to_string(),
            ))
            .await
            .map(|_| ())
        })
    })
    .await
    .unwrap();

    // rollback
    let _ = db
        .transaction::<_, (), sea_orm::DbErr>(|tx| {
            Box::pin(async move {
                tx.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "INSERT INTO t (n) VALUES (2)".to_string(),
                ))
                .await?;
                Err(sea_orm::DbErr::Custom("nope".to_string()))
            })
        })
        .await;

    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM t".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let count: i64 = row.try_get("", "c").unwrap();
    assert_eq!(count, 1, "only the committed insert should remain");
}
