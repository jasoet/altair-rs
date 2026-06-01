//! `altair-wf` — `run_dag` pattern with a diamond-shaped graph.
//!
//! Layout:
//! ```text
//!     build
//!     /   \
//!  test   lint
//!     \   /
//!     deploy
//! ```
//! `test` and `lint` run concurrently after `build` succeeds; `deploy`
//! waits on both. Topological layering is computed once at the start
//! and recorded in workflow history, so replay is deterministic.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_dag
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
    DAGInput, DAGNode, DAGOutput, TaskInput, TaskOutput, default_activity_options, run_dag,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepIn {
    pub stage: String,
}
impl TaskInput for StepIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepOut {
    pub artifact: String,
}
impl TaskOutput for StepOut {
    fn is_success(&self) -> bool {
        true
    }
}

pub struct StageActivities;

#[activities]
impl StageActivities {
    #[activity]
    pub async fn run_stage(
        _ctx: ActivityContext,
        input: StepIn,
    ) -> std::result::Result<StepOut, ActivityError> {
        Ok(StepOut {
            artifact: format!("{}.artifact", input.stage),
        })
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloDagWf;

#[workflow_methods]
impl HelloDagWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: DAGInput<StepIn>,
    ) -> WorkflowResult<DAGOutput<StepOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = run_dag(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(StageActivities::run_stage, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("StageActivities::run_stage", e))
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
        task_queue: "altair-wf-hello-dag".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloDagWf>()
        .register_activities(StageActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;

    let input = DAGInput {
        nodes: vec![
            DAGNode {
                name: "build".into(),
                input: StepIn {
                    stage: "build".into(),
                },
                dependencies: vec![],
            },
            DAGNode {
                name: "test".into(),
                input: StepIn {
                    stage: "test".into(),
                },
                dependencies: vec!["build".into()],
            },
            DAGNode {
                name: "lint".into(),
                input: StepIn {
                    stage: "lint".into(),
                },
                dependencies: vec!["build".into()],
            },
            DAGNode {
                name: "deploy".into(),
                input: StepIn {
                    stage: "deploy".into(),
                },
                dependencies: vec!["test".into(), "lint".into()],
            },
        ],
        fail_fast: true,
        max_parallel: 0,
    };
    let wf_id = format!("hello-dag-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloDagWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: DAGOutput<StepOut> = handle
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
    println!("  execution order (per topological layer):");
    for node in &out.node_results {
        println!(
            "    {} -> {}",
            node.name,
            node.result
                .as_ref()
                .map_or("<none>", |r| r.artifact.as_str())
        );
    }
    Ok(())
}
