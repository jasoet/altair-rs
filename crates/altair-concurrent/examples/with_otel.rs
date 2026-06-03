//! Cross-crate auto-integration with `altair-otel`.
//!
//! Each parallel task runs inside its own `concurrent.task` span. When
//! `altair-otel` is initialized in the same process, every task span flows
//! through the configured exporter — so a batch of three concurrent tasks
//! produces a parent `concurrent_run` span with three child task spans.
//!
//! Run with: `cargo run --example with_otel -p altair-concurrent`

use altair_concurrent::prelude::*;
use altair_otel::{Config, Exporter};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init altair-otel with the Stdout exporter so spans print to console.
    Config::builder()
        .service_name("concurrent-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    let tasks: TaskMap<String> = TaskMap::new()
        .insert("fetch_user", |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok::<_, std::io::Error>("alice".to_string())
        })
        .insert("fetch_orders", |_| async {
            tokio::time::sleep(Duration::from_millis(35)).await;
            Ok::<_, std::io::Error>("3 open".to_string())
        })
        .insert("fetch_prefs", |_| async {
            tokio::time::sleep(Duration::from_millis(15)).await;
            Ok::<_, std::io::Error>("dark mode".to_string())
        });

    let results = execute_concurrently(tasks).await?;
    println!("\nresults: {results:?}");

    // Drain pending spans before exit.
    altair_otel::shutdown();
    Ok(())
}
