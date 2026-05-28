//! Environment variables override file values. With `env_prefix("APP")`,
//! `APP_PORT` overrides the `port` field, and `APP_DATABASE_HOST` overrides
//! the nested `database.host` field.
//!
//! Run with:
//!   `cargo run --example env_override -p altair-config`
//! Or set your own overrides:
//!   `APP_PORT=9000 APP_DATABASE_HOST=db.prod cargo run --example env_override -p altair-config`

use altair_config::prelude::*;
use std::io::Write;

#[derive(Debug, Deserialize, Validate)]
#[allow(dead_code)] // fields are read via Debug formatting; rustc doesn't count that
struct DbConfig {
    #[validate(length(min = 1))]
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    #[validate(nested)]
    database: DbConfig,
}

fn main() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new()?;
    let base = dir.path().join("app.toml");
    std::fs::File::create(&base)?.write_all(
        br#"
port = 8080

[database]
host = "localhost"
port = 5432
"#,
    )?;

    let cfg: AppConfig = Loader::new().toml_file(&base).env_prefix("APP").build()?;

    println!("config: {cfg:#?}");
    println!();
    println!("try running again with env overrides:");
    println!("  APP_PORT=9090 cargo run --example env_override -p altair-config");
    println!("  APP_DATABASE_HOST=db.prod cargo run --example env_override -p altair-config");
    Ok(())
}
