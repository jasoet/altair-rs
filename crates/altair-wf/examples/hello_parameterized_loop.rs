//! `altair-wf` — `parameterized_loop` (cartesian product) pattern.
//!
//! Each `(region, tier)` combination produces one activity dispatch.
//! Iteration order is deterministic — keys sorted lexicographically
//! before the product is expanded — so the same input produces the
//! same dispatch order on every workflow replay.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_parameterized_loop
//! ```

#![allow(missing_docs, clippy::unused_async)]

use std::collections::HashMap;

use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_wf::{
    FailureStrategy, LoopOutput, ParameterizedLoopInput, Substitutor, TaskInput, TaskOutput,
    default_activity_options, parameterized_loop, substitutor_from_fn,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeployIn {
    pub region: String,
    pub tier: String,
}
impl TaskInput for DeployIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeployOut {
    pub message: String,
}
impl TaskOutput for DeployOut {
    fn is_success(&self) -> bool {
        true
    }
}

pub struct DeployActivities;

#[activities]
impl DeployActivities {
    #[activity]
    pub async fn deploy(
        _ctx: ActivityContext,
        input: DeployIn,
    ) -> std::result::Result<DeployOut, ActivityError> {
        Ok(DeployOut {
            message: format!("deployed {}/{}", input.region, input.tier),
        })
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloParameterizedLoopWf;

#[workflow_methods]
impl HelloParameterizedLoopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ParameterizedLoopInput<DeployIn>,
    ) -> WorkflowResult<LoopOutput<DeployOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let substitutor: Substitutor<DeployIn> =
            substitutor_from_fn(|_template: &DeployIn, _item: &str, _idx: usize, params| {
                DeployIn {
                    region: params.get("region").cloned().unwrap_or_default(),
                    tier: params.get("tier").cloned().unwrap_or_default(),
                }
            });
        let result = parameterized_loop(input, substitutor, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(DeployActivities::deploy, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("DeployActivities::deploy", e))
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
        task_queue: "altair-wf-hello-paramloop".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloParameterizedLoopWf>()
        .register_activities(DeployActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;

    let mut parameters: HashMap<String, Vec<String>> = HashMap::new();
    parameters.insert(
        "region".into(),
        vec!["us-east-1".into(), "eu-west-1".into()],
    );
    parameters.insert("tier".into(), vec!["standard".into(), "premium".into()]);
    let input = ParameterizedLoopInput {
        parameters,
        template: DeployIn {
            region: String::new(),
            tier: String::new(),
        },
        parallel: true,
        failure_strategy: FailureStrategy::Continue,
        max_in_flight: 0,
    };
    let wf_id = format!("hello-paramloop-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloParameterizedLoopWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: LoopOutput<DeployOut> = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!("workflow {wf_id} finished:");
    println!("  cartesian product size = {}", out.item_count);
    for step in &out.results {
        println!("  {}", step.message);
    }
    Ok(())
}
