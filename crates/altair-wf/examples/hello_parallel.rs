//! `altair-wf` — `parallel` pattern with `FailureStrategy`, end to end
//! against a real Temporal server.
//!
//! All three activities fan out at once via `join_all`. The chosen
//! `FailureStrategy::Continue` collects every result regardless of
//! per-task failure; switch to `FailFast` to short-circuit on the first
//! error.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_parallel
//! ```

#![allow(missing_docs, clippy::unused_async)]

use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
use altair_temporal::temporalio_common;
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_wf::{
    FailureStrategy, ParallelInput, ParallelOutput, TaskInput, TaskOutput,
    default_activity_options, parallel,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchIn {
    pub url: String,
    pub will_fail: bool,
}
impl TaskInput for FetchIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchOut {
    pub url: String,
    pub bytes: usize,
    pub ok: bool,
}
impl TaskOutput for FetchOut {
    fn is_success(&self) -> bool {
        self.ok
    }
    fn error(&self) -> Option<&str> {
        if self.ok { None } else { Some(&self.url) }
    }
}

pub struct FetchActivities;

#[activities]
impl FetchActivities {
    #[activity]
    pub async fn fetch(
        _ctx: ActivityContext,
        input: FetchIn,
    ) -> std::result::Result<FetchOut, ActivityError> {
        // Simulated work. A real activity would call reqwest etc.
        if input.will_fail {
            Err(ActivityError::application(
                temporalio_common::error::ApplicationFailure::builder(anyhow::anyhow!(
                    "fetch failed: {}",
                    input.url
                ))
                .type_name("FetchFailed".to_string())
                .non_retryable(true)
                .build(),
            ))
        } else {
            Ok(FetchOut {
                url: input.url.clone(),
                bytes: input.url.len() * 100,
                ok: true,
            })
        }
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloParallelWf;

#[workflow_methods]
impl HelloParallelWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ParallelInput<FetchIn>,
    ) -> WorkflowResult<ParallelOutput<FetchOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = parallel(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(FetchActivities::fetch, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("FetchActivities::fetch", e))
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
        task_queue: "altair-wf-hello-parallel".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloParallelWf>()
        .register_activities(FetchActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = ParallelInput {
        tasks: vec![
            FetchIn {
                url: "https://a.example".into(),
                will_fail: false,
            },
            FetchIn {
                url: "https://b.example".into(),
                will_fail: true,
            },
            FetchIn {
                url: "https://c.example".into(),
                will_fail: false,
            },
        ],
        failure_strategy: FailureStrategy::Continue,
    };
    let wf_id = format!("hello-parallel-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloParallelWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        // FailFast would surface a workflow failure here; Continue
        // returns the partial outcome we print below.
        let out: ParallelOutput<FetchOut> = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!("workflow {wf_id} finished:");
    println!(
        "  total_success = {}, total_failed = {}",
        out.total_success, out.total_failed,
    );
    for (i, step) in out.results.iter().enumerate() {
        println!(
            "  task[{i}] {} -> {} bytes (ok={})",
            step.url, step.bytes, step.ok,
        );
    }
    Ok(())
}
