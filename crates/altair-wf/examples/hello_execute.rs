//! `altair-wf` — `execute` (single task) pattern, end to end against
//! a real Temporal server.
//!
//! `execute` is the simplest pattern: dispatch one activity, return one
//! result. Useful when you want the framework's typed dispatch and
//! error wrapping but don't need orchestration.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_execute
//! ```

#![allow(missing_docs, clippy::unused_async)]

use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_wf::{TaskInput, TaskOutput, default_activity_options, execute};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SquareIn {
    pub value: i64,
}
impl TaskInput for SquareIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SquareOut {
    pub squared: i64,
}
impl TaskOutput for SquareOut {
    fn is_success(&self) -> bool {
        true
    }
}

pub struct MathActivities;

#[activities]
impl MathActivities {
    #[activity]
    pub async fn square(
        _ctx: ActivityContext,
        input: SquareIn,
    ) -> std::result::Result<SquareOut, ActivityError> {
        Ok(SquareOut {
            squared: input.value * input.value,
        })
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloExecuteWf;

#[workflow_methods]
impl HelloExecuteWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: SquareIn,
    ) -> WorkflowResult<SquareOut> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = execute(input, |task| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(MathActivities::square, task, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("MathActivities::square", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    let cfg = altair_temporal::Config {
        task_queue: "altair-wf-hello-execute".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloExecuteWf>()
        .register_activities(MathActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = SquareIn { value: 7 };
    let wf_id = format!("hello-execute-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloExecuteWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: SquareOut = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!("workflow {wf_id} finished: 7 * 7 = {}", out.squared);
    Ok(())
}
