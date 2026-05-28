//! Each task runs inside a `tracing::info_span!("concurrent.task", task.name = ...)`.
//! With a tracing subscriber installed, you see one span per task plus an
//! aggregate batch span with `task_count`.
//!
//! Run with: `RUST_LOG=info cargo run --example traced_tasks -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> altair_concurrent::Result<()> {
    // Install a simple text subscriber so we can see the spans + events.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let tasks: TaskMap<u32> = TaskMap::new()
        .insert("fetch_user", |_| async {
            info!("starting user fetch");
            tokio::time::sleep(Duration::from_millis(20)).await;
            info!("user fetch complete");
            Ok::<_, std::io::Error>(42)
        })
        .insert("fetch_prefs", |_| async {
            info!("starting prefs fetch");
            tokio::time::sleep(Duration::from_millis(15)).await;
            info!("prefs fetch complete");
            Ok::<_, std::io::Error>(7)
        });

    let results = execute_concurrently(tasks).await?;
    println!("results: {results:?}");

    Ok(())
}
