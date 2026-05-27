//! Run with: `cargo run --example basic -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("fetch_user", |_| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<_, std::io::Error>("alice".to_string())
        })
        .insert("fetch_orders", |_| async {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok::<_, std::io::Error>("3 open".to_string())
        })
        .insert("fetch_prefs", |_| async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok::<_, std::io::Error>("dark mode".to_string())
        });

    let results = execute_concurrently(tasks).await?;
    for (name, value) in results {
        println!("{name} = {value}");
    }
    Ok(())
}
