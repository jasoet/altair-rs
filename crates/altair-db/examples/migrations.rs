//! Run a set of SQL migrations from an in-process tempdir.
//!
//! Run with: `cargo run --example migrations -p altair-db`

use std::io::Write;

use altair_db::prelude::*;
use sea_orm::{ConnectionTrait, Statement};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut f = std::fs::File::create(dir.path().join("20260101000000_create_widgets.sql"))?;
    writeln!(
        f,
        "CREATE TABLE widgets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);"
    )?;
    let mut g = std::fs::File::create(dir.path().join("20260102000000_add_qty.sql"))?;
    writeln!(g, "ALTER TABLE widgets ADD COLUMN qty INTEGER DEFAULT 0;")?;
    drop((f, g));

    let db = Db::connect(Config::from_url("sqlite::memory:")).await?;
    db.migrate(dir.path()).await?;

    let backend = db.orm().get_database_backend();
    let row = db
        .orm()
        .query_one(Statement::from_string(
            backend,
            "SELECT COUNT(*) AS c FROM _sqlx_migrations".to_string(),
        ))
        .await?
        .expect("count");
    let n: i64 = row.try_get("", "c")?;
    println!("applied {n} migrations");
    Ok(())
}
