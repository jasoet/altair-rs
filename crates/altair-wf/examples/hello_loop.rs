//! `altair-wf` — `run_loop` per-item pattern with a substitutor.
//!
//! The substitutor takes a template input + (item, index) and produces
//! the concrete per-iteration input. Set `parallel = true` to fan out;
//! `false` to run sequentially.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --example hello_loop
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
    FailureStrategy, LoopInput, LoopOutput, Substitutor, TaskInput, TaskOutput,
    default_activity_options, run_loop, substitutor_from_fn,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessIn {
    pub template_prefix: String,
    pub item: String,
    pub index: usize,
}
impl TaskInput for ProcessIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessOut {
    pub label: String,
}
impl TaskOutput for ProcessOut {
    fn is_success(&self) -> bool {
        true
    }
}

pub struct ProcessActivities;

#[activities]
impl ProcessActivities {
    #[activity]
    pub async fn process(
        _ctx: ActivityContext,
        input: ProcessIn,
    ) -> std::result::Result<ProcessOut, ActivityError> {
        Ok(ProcessOut {
            label: format!(
                "[{}] {}: {}",
                input.index, input.template_prefix, input.item
            ),
        })
    }
}

#[workflow]
#[derive(Default)]
pub struct HelloLoopWf;

#[workflow_methods]
impl HelloLoopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: LoopInput<ProcessIn>,
    ) -> WorkflowResult<LoopOutput<ProcessOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let substitutor: Substitutor<ProcessIn> =
            substitutor_from_fn(|template: &ProcessIn, item: &str, idx: usize, _params| {
                ProcessIn {
                    template_prefix: template.template_prefix.clone(),
                    item: item.to_string(),
                    index: idx,
                }
            });
        let result = run_loop(input, substitutor, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(ProcessActivities::process, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("ProcessActivities::process", e))
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
        task_queue: "altair-wf-hello-loop".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<HelloLoopWf>()
        .register_activities(ProcessActivities)
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = LoopInput {
        items: vec!["apple".into(), "banana".into(), "cherry".into()],
        template: ProcessIn {
            template_prefix: "fruit".into(),
            item: String::new(),
            index: 0,
        },
        parallel: true,
        failure_strategy: FailureStrategy::Continue,
    };
    let wf_id = format!("hello-loop-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                HelloLoopWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: LoopOutput<ProcessOut> = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!("workflow {wf_id} finished:");
    println!("  iterations = {}", out.item_count);
    for step in &out.results {
        println!("  {}", step.label);
    }
    Ok(())
}
