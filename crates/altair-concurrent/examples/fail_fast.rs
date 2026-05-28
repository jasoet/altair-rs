//! Fail-fast semantics: the first task error cancels remaining tasks and
//! surfaces as `Error::TaskFailed`.
//!
//! Run with: `cargo run --example fail_fast -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let tasks: TaskMap<u32> = TaskMap::new()
        .insert("alpha", |_| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<_, std::io::Error>(1)
        })
        .insert("bravo", |_| async {
            // This one fails — bravo is the loser.
            Err::<u32, _>(std::io::Error::other(
                "upstream service rejected the request",
            ))
        })
        .insert("charlie", |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, std::io::Error>(3)
        });

    match execute_concurrently(tasks).await {
        Ok(results) => println!("all succeeded: {results:?}"),
        Err(Error::TaskFailed { name, source }) => {
            println!("task '{name}' failed: {source}");
            println!("(charlie was cancelled before its sleep finished)");
        }
        Err(other) => println!("infrastructure error: {other}"),
    }
}
