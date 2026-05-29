//! Load `altair_db::Config` from a TOML file via serde.
//!
//! Run with: `cargo run --example with_config -p altair-db`

use std::io::Write;

use altair_db::prelude::*;

#[derive(serde::Deserialize)]
struct AppConfig {
    database: Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db_example.toml");
    let mut f = std::fs::File::create(&path)?;
    writeln!(
        f,
        r#"[database]
url = "sqlite::memory:"
max_connections = 5
acquire_timeout = "5s"
sqlx_slow_query_threshold = "500ms"
"#
    )?;
    drop(f);

    let raw = std::fs::read_to_string(&path)?;
    let cfg: AppConfig = toml::from_str(&raw)?;
    let db = Db::connect(cfg.database).await?;
    db.ping().await?;
    println!("connected: backend={:?}", db.backend());
    Ok(())
}
