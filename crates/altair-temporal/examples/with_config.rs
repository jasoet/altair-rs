//! Load [`Config`] from TOML and connect.
//!
//! Run with: `cargo run -p altair-temporal --example with_config`

use std::io::Write;

use altair_temporal::prelude::*;

#[derive(serde::Deserialize)]
struct AppConfig {
    temporal: Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("temporal.toml");
    let mut f = std::fs::File::create(&path)?;
    writeln!(
        f,
        r#"[temporal]
host = "http://localhost:7233"
namespace = "default"
task_queue = "altair-demo"
identity = "altair-temporal-example"
max_concurrent_activities = 50
max_concurrent_workflows = 50
"#
    )?;
    drop(f);

    let raw = std::fs::read_to_string(&path)?;
    let app: AppConfig = toml::from_str(&raw)?;
    println!(
        "loaded config: host={} namespace={} tq={}",
        app.temporal.host, app.temporal.namespace, app.temporal.task_queue
    );

    let _client = Client::from_config(&app.temporal).await?;
    println!("connected.");
    Ok(())
}
