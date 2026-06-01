//! `altair-wf` — `pipeline` (sequential) pattern, end to end against
//! a real Temporal server.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_pipeline
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
use altair_wf::{
    PipelineInput, PipelineOutput, TaskInput, TaskOutput, default_activity_options, pipeline,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GreetIn {
    pub name: String,
}
impl TaskInput for GreetIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GreetOut {
    pub message: String,
    pub ok: bool,
}
impl TaskOutput for GreetOut {
    fn is_success(&self) -> bool {
        self.ok
    }
}

pub struct GreetActivities;

#[activities]
impl GreetActivities {
    #[activity]
    pub async fn greet(
        _ctx: ActivityContext,
        input: GreetIn,
    ) -> std::result::Result<GreetOut, ActivityError> {
        Ok(GreetOut {
            message: format!("hello, {}!", input.name),
            ok: true,
        })
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloPipelineWf;

#[workflow_methods]
impl HelloPipelineWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: PipelineInput<GreetIn>,
    ) -> WorkflowResult<PipelineOutput<GreetOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = pipeline(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(GreetActivities::greet, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("GreetActivities::greet", e))
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
        task_queue: "altair-wf-hello-pipeline".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloPipelineWf>()
        .register_activities(GreetActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = PipelineInput {
        tasks: vec![
            GreetIn {
                name: "alice".into(),
            },
            GreetIn { name: "bob".into() },
            GreetIn {
                name: "carol".into(),
            },
        ],
        stop_on_error: true,
        cleanup: false,
    };
    let wf_id = format!("hello-pipeline-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    // The Temporal SDK's worker future is not `Send`, so we cannot
    // `tokio::spawn` it. Instead, drive the worker and the client-side
    // workload concurrently on the same task via
    // `futures::future::join`, and signal the worker to stop once the
    // workload finishes.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloPipelineWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: PipelineOutput<GreetOut> = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!("workflow {wf_id} finished:");
    println!("  total_success = {}", out.total_success);
    println!("  total_failed  = {}", out.total_failed);
    for (i, step) in out.results.iter().enumerate() {
        println!("  step[{i}]: {}", step.message);
    }
    Ok(())
}
