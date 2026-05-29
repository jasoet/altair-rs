//! Minimal worker startup. Requires a running Temporal server at
//! `http://localhost:7233` to actually execute; otherwise it errors at
//! connect time.
//!
//! Run with: `cargo run -p altair-temporal --example basic_worker`

use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config {
        task_queue: "altair-demo".to_string(),
        ..Config::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        // .register_workflow::<MyWorkflow>()
        // .register_activities(MyActivities)
        .build()
        .await?;
    println!("worker built; polling task_queue={}", cfg.task_queue);
    worker.run().await?;
    Ok(())
}
