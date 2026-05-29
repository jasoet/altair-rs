//! Create a daily cron schedule.
//!
//! Run with: `cargo run -p altair-temporal --example schedule_cron`

use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config {
        task_queue: "altair-demo".to_string(),
        ..Config::default()
    };

    let client = Client::from_config(&cfg).await?;

    Schedule::builder()
        .cron("0 9 * * *")
        .note("daily archive at 09:00 UTC")
        .start_workflow("ArchiveWorkflow", &cfg.task_queue, "archive-daily")
        .create(&client, "daily-archive")
        .await?;

    println!("schedule created: daily-archive");
    Ok(())
}
