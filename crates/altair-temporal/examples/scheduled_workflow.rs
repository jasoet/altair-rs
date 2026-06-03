//! `altair-temporal` — register a schedule that runs a small workflow
//! every 2 minutes, then start a worker so the executions actually
//! fire.
//!
//! Run alongside `temporal server start-dev`:
//!
//! ```bash
//! # Terminal 1: dev server (UI at http://localhost:8233)
//! temporal server start-dev
//!
//! # Terminal 2: this example. Will hold the worker open; Ctrl-C exits.
//! cargo run -p altair-temporal --example scheduled_workflow
//! ```
//!
//! Inspect:
//! - http://localhost:8233/namespaces/default/schedules
//! - http://localhost:8233/namespaces/default/workflows
//!
//! Schedule id and workflow id stay stable across runs so the example
//! is safe to re-run (uses `create_or_update`).

#![allow(missing_docs, clippy::unused_async, clippy::doc_markdown)]

use std::time::Duration;

use altair_temporal::temporalio_client::Client;
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{WorkflowContext, WorkflowResult};
use altair_temporal::{Config, Schedule, WorkerBuilder};

#[workflow]
#[derive(Default)]
pub struct HeartbeatWf;

#[workflow_methods]
impl HeartbeatWf {
    #[run]
    pub async fn run(_ctx: &mut WorkflowContext<Self>, _input: ()) -> WorkflowResult<String> {
        Ok("scheduled heartbeat ok".to_string())
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let cfg = Config {
        task_queue: "altair-temporal-scheduled".to_string(),
        ..Config::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HeartbeatWf>()
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;

    Schedule::builder()
        .interval(Duration::from_mins(2))
        .note("altair-temporal: 2-minute heartbeat workflow")
        .start_workflow(
            "HeartbeatWf",
            &cfg.task_queue,
            "altair-temporal-heartbeat-recurring",
        )
        .create_or_update(&client, "altair-temporal-heartbeat")
        .await?;

    println!();
    println!("registered schedule altair-temporal-heartbeat (interval 2m)");
    println!();
    println!("Temporal UI:");
    println!("  schedules: http://localhost:8233/namespaces/default/schedules");
    println!("  workflows: http://localhost:8233/namespaces/default/workflows");
    println!();
    println!("worker running — Ctrl-C to stop.");
    println!();

    worker.run().await?;
    Ok(())
}
