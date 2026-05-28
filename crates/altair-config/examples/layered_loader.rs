//! Layered loading: a required base file + an optional local override.
//! Later sources override earlier ones.
//!
//! Run with: `cargo run --example layered_loader -p altair-config`

use altair_config::prelude::*;
use std::io::Write;

#[derive(Debug, Deserialize, Validate)]
struct DbConfig {
    #[validate(length(min = 1))]
    host: String,
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    #[validate(length(min = 1))]
    name: String,
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
    let base = dir.path().join("base.toml");
    let local = dir.path().join("local.toml");

    std::fs::File::create(&base)?.write_all(
        br#"
port = 8080

[database]
host = "localhost"
port = 5432
name = "prod"
"#,
    )?;

    std::fs::File::create(&local)?.write_all(
        br#"
[database]
host = "127.0.0.1"
name = "dev_local"
"#,
    )?;

    let cfg: AppConfig = Loader::new()
        .toml_file(&base)
        .toml_file_optional(&local)
        .build()?;

    println!("merged config: {cfg:#?}");
    println!();
    println!("note: 'database.port' came from base.toml (5432)");
    println!("note: 'database.host' and 'database.name' came from local.toml (override)");
    Ok(())
}
