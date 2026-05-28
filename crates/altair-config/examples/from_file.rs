//! Load a config from a TOML file path. Demonstrates the `from_file` helper
//! and creates the fixture file in a temp directory so the example runs
//! anywhere.
//!
//! Run with: `cargo run --example from_file -p altair-config`

use altair_config::prelude::*;
use std::io::Write;

#[derive(Debug, Deserialize, Validate)]
struct AppConfig {
    #[validate(range(min = 1, max = 65535))]
    port: u16,
    #[validate(length(min = 1))]
    name: String,
}

fn main() -> anyhow::Result<()> {
    // Create the fixture file in a tempdir so we don't depend on cwd.
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("app.toml");
    std::fs::File::create(&path)?.write_all(
        br#"
port = 9090
name = "checkout-api"
"#,
    )?;

    let cfg: AppConfig = from_file(&path, "APP")?;
    println!("loaded from {}: {cfg:#?}", path.display());
    Ok(())
}
