//! `MySQL` integration via testcontainers. Requires Docker.

#![cfg(feature = "mysql")]

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

async fn start() -> (Db, testcontainers::ContainerAsync<Mysql>) {
    let container = Mysql::default().start().await.expect("start mysql");
    let port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("mysql port");
    let url = format!("mysql://root@127.0.0.1:{port}/test");
    let db = Db::connect(Config::from_url(url)).await.expect("connect");
    (db, container)
}

#[tokio::test]
async fn connects_and_reports_mysql_backend() {
    let (db, _c) = start().await;
    assert_eq!(db.backend(), Backend::MySql);
    db.ping().await.unwrap();
}

#[tokio::test]
async fn raw_sqlx_query_via_pool() {
    let (db, _c) = start().await;
    let pool = db.mysql_pool().expect("mysql pool");
    let (one,): (i64,) = sqlx::query_as("SELECT 1").fetch_one(pool).await.unwrap();
    assert_eq!(one, 1);
}

#[tokio::test]
async fn orm_round_trip() {
    let (db, _c) = start().await;
    let backend = db.orm().get_database_backend();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "CREATE TABLE widgets (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(64) NOT NULL)"
                .to_string(),
        ))
        .await
        .unwrap();
    db.orm()
        .execute(Statement::from_string(
            backend,
            "INSERT INTO widgets (name) VALUES ('a'), ('b'), ('c')".to_string(),
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
    assert_eq!(count, 3);
}
