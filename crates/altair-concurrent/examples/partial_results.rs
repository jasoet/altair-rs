//! Partial-results mode: every task runs to completion; per-task outcomes
//! appear in a `HashMap<&str, Result<T, BoxedError>>`.
//!
//! Use this when you need ALL tasks' results regardless of who failed —
//! e.g. running independent health checks and rendering a status page.
//!
//! Run with: `cargo run --example partial_results -p altair-concurrent`

use altair_concurrent::prelude::*;

#[tokio::main]
async fn main() -> altair_concurrent::Result<()> {
    let tasks: TaskMap<&'static str> = TaskMap::new()
        .insert("db", |_| async {
            // Healthy
            Ok::<_, std::io::Error>("connected")
        })
        .insert("cache", |_| async {
            // Down
            Err::<&'static str, _>(std::io::Error::other("redis timeout"))
        })
        .insert("payment_gw", |_| async {
            Ok::<_, std::io::Error>("3 active connections")
        })
        .insert("storage", |_| async {
            Err::<&'static str, _>(std::io::Error::other("S3 unreachable"))
        });

    let results: PartialResults<&'static str> =
        execute_concurrently(tasks).with_partial_results().await?;

    println!("Service health:");
    let mut names: Vec<_> = results.keys().collect();
    names.sort();
    for name in names {
        match &results[name] {
            Ok(status) => println!("  [OK]   {name}: {status}"),
            Err(e) => println!("  [DOWN] {name}: {e}"),
        }
    }

    Ok(())
}
