//! `altair-wf` — `function` feature: register handlers by name, then
//! drive a pipeline of named jobs through a single Temporal activity.
//!
//! The `Registry` holds `name -> async fn(FunctionInput) -> Result<FunctionOutput>`
//! mappings. A workflow body uses the `pipeline` (or `parallel`, etc.)
//! pattern with `FunctionActivities::execute_function` as the activity;
//! each `FunctionExecutionInput` names the handler the activity should
//! dispatch to.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --features function --example function_registry
//! ```

#![allow(missing_docs, clippy::unused_async)]

use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{WorkflowContext, WorkflowResult};
use altair_wf::function::{
    FunctionActivities, FunctionExecutionInput, FunctionExecutionOutput, FunctionInput,
    FunctionOutput, Registry,
};
use altair_wf::{PipelineInput, PipelineOutput, default_activity_options, pipeline};

#[workflow]
#[derive(Default)]
pub struct FunctionPipelineWf;

#[workflow_methods]
impl FunctionPipelineWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: PipelineInput<FunctionExecutionInput>,
    ) -> WorkflowResult<PipelineOutput<FunctionExecutionOutput>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = pipeline(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(FunctionActivities::execute_function, step, opts)
                    .await
                    .map_err(|e| {
                        altair_wf::Error::activity("FunctionActivities::execute_function", e)
                    })
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

fn build_registry() -> Registry {
    let mut reg = Registry::new();
    reg.register("upper", |input: FunctionInput| async move {
        let text = input.args.get("text").cloned().unwrap_or_default();
        Ok::<_, std::io::Error>(FunctionOutput::with_result([(
            "out".to_string(),
            text.to_uppercase(),
        )]))
    })
    .expect("register upper");
    reg.register("repeat", |input: FunctionInput| async move {
        let text = input.args.get("text").cloned().unwrap_or_default();
        let n: usize = input
            .args
            .get("count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        Ok::<_, std::io::Error>(FunctionOutput::with_result([(
            "out".to_string(),
            text.repeat(n),
        )]))
    })
    .expect("register repeat");
    reg.register("reverse", |input: FunctionInput| async move {
        let text = input.args.get("text").cloned().unwrap_or_default();
        Ok::<_, std::io::Error>(FunctionOutput::with_result([(
            "out".to_string(),
            text.chars().rev().collect::<String>(),
        )]))
    })
    .expect("register reverse");
    reg
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    let cfg = altair_temporal::Config {
        task_queue: "altair-wf-function-registry".to_string(),
        ..Default::default()
    };

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<FunctionPipelineWf>()
        .register_activities(FunctionActivities::new(build_registry()))
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = PipelineInput {
        tasks: vec![
            FunctionExecutionInput::new("upper").with_args([("text", "hello world")]),
            FunctionExecutionInput::new("repeat").with_args([("text", "ab"), ("count", "3")]),
            FunctionExecutionInput::new("reverse").with_args([("text", "altair-wf")]),
        ],
        stop_on_error: false,
        cleanup: false,
    };
    let wf_id = format!("function-registry-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                FunctionPipelineWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        let out: PipelineOutput<FunctionExecutionOutput> = handle
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
    for step in &out.results {
        let out_value = step.result.get("out").cloned().unwrap_or_default();
        println!("  {} -> {out_value}", step.name);
    }
    Ok(())
}
