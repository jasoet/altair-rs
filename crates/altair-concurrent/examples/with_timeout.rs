//! Batch timeout: if any task hasn't finished by the deadline, the whole
//! batch is cancelled and `Error::Timeout` is returned.
//!
//! Run with: `cargo run --example with_timeout -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let tasks: TaskMap<u32> = TaskMap::new()
        .insert("fast", |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok::<_, std::io::Error>(1)
        })
        .insert("slow", |_| async {
            // Will be cancelled by the timeout before completing.
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, std::io::Error>(2)
        });

    let started = std::time::Instant::now();
    let result = execute_concurrently(tasks)
        .with_timeout(Duration::from_millis(100))
        .await;
    let elapsed = started.elapsed();

    match result {
        Ok(map) => println!("(unexpected) succeeded: {map:?}"),
        Err(Error::Timeout) => {
            println!("batch timed out after {elapsed:?} (limit was 100ms)");
        }
        Err(other) => println!("other error: {other}"),
    }
}
