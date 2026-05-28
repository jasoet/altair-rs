//! External cancellation: pass a `CancellationToken`; cancelling it from
//! anywhere (signal handler, parent task, watchdog) aborts the batch.
//!
//! Run with: `cargo run --example with_cancellation -p altair-concurrent`

use altair_concurrent::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();

    let tasks: TaskMap<()> = TaskMap::new().insert("worker", |ct| async move {
        // Cooperative: select on the cancellation token AND the work.
        tokio::select! {
            () = ct.cancelled() => {
                println!("worker: observed cancellation");
                Err::<(), _>(std::io::Error::other("cancelled by caller"))
            }
            () = tokio::time::sleep(Duration::from_secs(30)) => {
                println!("worker: would have finished in 30s");
                Ok(())
            }
        }
    });

    // Simulate cancel from another task — e.g. SIGTERM handler.
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        println!("controller: cancelling");
        canceller.cancel();
    });

    let result = execute_concurrently(tasks).with_cancellation(cancel).await;
    match result {
        Ok(_) => println!("main: succeeded (unexpected)"),
        Err(Error::TaskFailed { name, source }) => {
            println!("main: '{name}' returned an error: {source}");
        }
        Err(Error::Cancelled) => println!("main: batch was cancelled"),
        Err(other) => println!("main: other error: {other}"),
    }
}
