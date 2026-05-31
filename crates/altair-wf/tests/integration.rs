//! End-to-end behaviour tests against a real Temporal server.
//!
//! Each test defines workflow types that use the `altair-wf` patterns
//! inside their `#[run]` methods, spins up a `TemporalContainer`, runs
//! the workflow, and asserts the result. Gated behind the
//! `integration-tests` feature.

#![cfg(feature = "integration-tests")]
#![allow(
    tail_expr_drop_order,
    clippy::missing_panics_doc,
    clippy::large_futures,
    clippy::duration_suboptimal_units,
    missing_docs,
    clippy::needless_pass_by_value,
    clippy::default_trait_access,
    clippy::unused_async,
    clippy::module_name_repetitions
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// Note: avoid `use altair_wf::prelude::*` here because the prelude exports
// a `Result<T>` 1-arg alias that would shadow `std::result::Result<T, E>`
// inside `#[activity]` and `#[workflow_methods]` macro expansions.
use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
use altair_temporal::temporalio_common;
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_temporal::testcontainer::TemporalContainer;
use altair_wf::{
    DAGInput, DAGNode, DAGOutput, FailureStrategy, LoopInput, LoopOutput, ParallelInput,
    ParallelOutput, ParameterizedLoopInput, PipelineInput, PipelineOutput, Substitutor, TaskInput,
    TaskOutput, default_activity_options, execute, parallel, parameterized_loop, pipeline, run_dag,
    run_loop, substitutor_from_fn,
};
#[allow(unused_imports)]
use futures::FutureExt as _;
use tokio::sync::OnceCell;

// ---------------------------------------------------------------------------
// Shared container fixture
// ---------------------------------------------------------------------------

static CONTAINER: OnceCell<TemporalContainer> = OnceCell::const_new();

async fn temporal() -> &'static TemporalContainer {
    CONTAINER
        .get_or_init(|| async {
            TemporalContainer::start()
                .await
                .expect("start Temporal container")
        })
        .await
}

fn unique(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    format!("{prefix}-{pid}-{n}")
}

async fn run_with_workload<F, T>(
    worker: altair_temporal::Worker,
    workload: F,
    deadline: Duration,
) -> T
where
    F: std::future::Future<Output = T>,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    let shutdown = async move {
        let _ = rx.await;
    };
    let worker_fut = Box::pin(worker.run_with_shutdown(shutdown));

    let workload_with_signal = Box::pin(async move {
        let result = workload.await;
        let _ = tx.send(());
        result
    });

    let (_, result) = tokio::time::timeout(
        deadline,
        futures::future::join(worker_fut, workload_with_signal),
    )
    .await
    .expect("worker + workload finish before deadline");
    result
}

// ---------------------------------------------------------------------------
// Common payloads + a single Echo activity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EchoIn {
    pub id: u32,
    pub msg: String,
    pub will_fail: bool,
}
impl TaskInput for EchoIn {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EchoOut {
    pub id: u32,
    pub echoed: String,
    pub ok: bool,
}
impl TaskOutput for EchoOut {
    fn is_success(&self) -> bool {
        self.ok
    }
    fn error(&self) -> Option<&str> {
        if self.ok { None } else { Some(&self.echoed) }
    }
}

pub struct EchoActivities;

#[activities]
impl EchoActivities {
    #[activity]
    pub async fn echo(
        _ctx: ActivityContext,
        input: EchoIn,
    ) -> std::result::Result<EchoOut, ActivityError> {
        if input.will_fail {
            Err(ActivityError::application(
                temporalio_common::error::ApplicationFailure::builder(anyhow::anyhow!(
                    "id={}", input.id
                ))
                .type_name("EchoFailed".to_string())
                .non_retryable(true)
                .build(),
            ))
        } else {
            Ok(EchoOut {
                id: input.id,
                echoed: format!("echo:{}", input.msg),
                ok: true,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Workflows — one per pattern
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct PipelineWf;

#[workflow_methods]
impl PipelineWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: PipelineInput<EchoIn>,
    ) -> WorkflowResult<PipelineOutput<EchoOut>> {
        let opts = default_activity_options();
        // Reborrow as a shared reference so the dispatch closure (Fn /
        // FnMut bound by altair_wf::pipeline) can be called repeatedly.
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = pipeline(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ParallelWf;

#[workflow_methods]
impl ParallelWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ParallelInput<EchoIn>,
    ) -> WorkflowResult<ParallelOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = parallel(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct DAGWf;

#[workflow_methods]
impl DAGWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: DAGInput<EchoIn>,
    ) -> WorkflowResult<DAGOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = run_dag(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ExecuteWf;

#[workflow_methods]
impl ExecuteWf {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: EchoIn) -> WorkflowResult<EchoOut> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = execute(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct LoopWf;

#[workflow_methods]
impl LoopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: LoopInput<EchoIn>,
    ) -> WorkflowResult<LoopOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let substitutor: Substitutor<EchoIn> = substitutor_from_fn(
            |template: &EchoIn, item: &str, idx: usize, _params| EchoIn {
                id: template.id + u32::try_from(idx).unwrap_or(0),
                msg: format!("{}-{item}", template.msg),
                will_fail: template.will_fail,
            },
        );
        let result = run_loop(input, substitutor, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ParameterizedLoopWf;

#[workflow_methods]
impl ParameterizedLoopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ParameterizedLoopInput<EchoIn>,
    ) -> WorkflowResult<LoopOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let substitutor: Substitutor<EchoIn> =
            substitutor_from_fn(|template: &EchoIn, _item: &str, idx: usize, params| {
                let region = params.get("region").cloned().unwrap_or_default();
                let tier = params.get("tier").cloned().unwrap_or_default();
                EchoIn {
                    id: template.id + u32::try_from(idx).unwrap_or(0),
                    msg: format!("{region}-{tier}"),
                    will_fail: template.will_fail,
                }
            });
        let result = parameterized_loop(input, substitutor, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn ok(id: u32) -> EchoIn {
    EchoIn {
        id,
        msg: format!("msg{id}"),
        will_fail: false,
    }
}
fn bad(id: u32) -> EchoIn {
    EchoIn {
        id,
        msg: format!("msg{id}"),
        will_fail: true,
    }
}

async fn build_worker(tq: &str) -> altair_temporal::Worker {
    let temporal = temporal().await;
    let cfg = temporal.config(tq);
    WorkerBuilder::new(&cfg)
        .register_workflow::<PipelineWf>()
        .register_workflow::<ParallelWf>()
        .register_workflow::<DAGWf>()
        .register_workflow::<ExecuteWf>()
        .register_workflow::<LoopWf>()
        .register_workflow::<ParameterizedLoopWf>()
        .register_activities(EchoActivities)
        .build()
        .await
        .expect("build worker")
}

async fn temporal_client() -> Client {
    let temporal = temporal().await;
    let cfg = temporal.config(unique("client"));
    altair_temporal::Client::from_config(&cfg)
        .await
        .expect("client")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pipeline_round_trip_three_tasks_continue() {
    let tq = unique("wf-pipeline-cont");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("pipeline-cont-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = PipelineInput {
            tasks: vec![ok(1), bad(2), ok(3)],
            stop_on_error: false,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                PipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: PipelineOutput<EchoOut> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.total_success, 2);
    assert_eq!(out.total_failed, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_round_trip_all_succeed() {
    let tq = unique("wf-parallel-all");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("parallel-all-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = ParallelInput {
            tasks: vec![ok(1), ok(2), ok(3), ok(4)],
            failure_strategy: FailureStrategy::Continue,
        };
        let handle = client
            .start_workflow(
                ParallelWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: ParallelOutput<EchoOut> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.total_success, 4);
    assert_eq!(out.total_failed, 0);
    assert_eq!(out.results.len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dag_diamond_runs_in_topological_order() {
    let tq = unique("wf-dag-diamond");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("dag-diamond-wid");
    let tq_clone = tq.clone();

    let input = DAGInput {
        nodes: vec![
            DAGNode {
                name: "a".into(),
                input: ok(1),
                dependencies: vec![],
            },
            DAGNode {
                name: "b".into(),
                input: ok(2),
                dependencies: vec!["a".into()],
            },
            DAGNode {
                name: "c".into(),
                input: ok(3),
                dependencies: vec!["a".into()],
            },
            DAGNode {
                name: "d".into(),
                input: ok(4),
                dependencies: vec!["b".into(), "c".into()],
            },
        ],
        fail_fast: true,
        max_parallel: 0,
    };

    let workload = async move {
        let handle = client
            .start_workflow(
                DAGWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: DAGOutput<EchoOut> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.total_success, 4);
    assert_eq!(out.total_failed, 0);
    assert_eq!(out.results.len(), 4);
    assert_eq!(out.node_results.len(), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pipeline_stop_on_error_returns_workflow_failure() {
    let tq = unique("wf-pipeline-stop");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("pipeline-stop-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = PipelineInput {
            tasks: vec![ok(1), bad(2), ok(3)],
            stop_on_error: true,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                PipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        let res: Result<PipelineOutput<EchoOut>, _> =
            handle.get_result(WorkflowGetResultOptions::default()).await;
        res
    };

    let res = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert!(res.is_err(), "expected workflow failure, got Ok");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_fail_fast_returns_workflow_failure() {
    let tq = unique("wf-parallel-fail-fast");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("parallel-ff-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = ParallelInput {
            tasks: vec![ok(1), bad(2), ok(3), ok(4)],
            failure_strategy: FailureStrategy::FailFast,
        };
        let handle = client
            .start_workflow(
                ParallelWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        let res: Result<ParallelOutput<EchoOut>, _> =
            handle.get_result(WorkflowGetResultOptions::default()).await;
        res
    };

    let res = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert!(res.is_err(), "expected workflow failure under fail-fast");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn execute_single_task_round_trip() {
    let tq = unique("wf-execute");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("execute-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let handle = client
            .start_workflow(
                ExecuteWf::run,
                ok(42),
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: EchoOut = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.id, 42);
    assert!(out.ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn run_loop_iterates_over_items() {
    let tq = unique("wf-loop");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("loop-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = LoopInput {
            items: vec!["us".into(), "eu".into(), "ap".into()],
            template: ok(100),
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
        };
        let handle = client
            .start_workflow(
                LoopWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: LoopOutput<EchoOut> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.item_count, 3);
    assert_eq!(out.total_success, 3);
    assert_eq!(out.total_failed, 0);
    assert_eq!(out.results.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parameterized_loop_cartesian_product_round_trip() {
    let tq = unique("wf-param-loop");
    let worker = build_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("param-loop-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let mut params = std::collections::HashMap::new();
        params.insert(
            "region".to_string(),
            vec!["us-east-1".into(), "eu-west-1".into()],
        );
        params.insert(
            "tier".to_string(),
            vec!["std".into(), "premium".into(), "enterprise".into()],
        );
        let input = ParameterizedLoopInput {
            parameters: params,
            template: ok(200),
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
        };
        let handle = client
            .start_workflow(
                ParameterizedLoopWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: LoopOutput<EchoOut> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    // 2 regions × 3 tiers = 6 combinations.
    assert_eq!(out.item_count, 6);
    assert_eq!(out.total_success, 6);
    assert_eq!(out.results.len(), 6);
}

// ---------------------------------------------------------------------------
// Function module (Phase 2): registry-based named-handler dispatch through
// a real Temporal worker.
// ---------------------------------------------------------------------------

use altair_wf::function::{
    FunctionActivities, FunctionExecutionInput, FunctionExecutionOutput, FunctionInput,
    FunctionOutput, Registry,
};

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

fn make_function_activities() -> FunctionActivities {
    let mut reg = Registry::new();
    reg.register("upper", |input: FunctionInput| async move {
        let v = input.args.get("text").cloned().unwrap_or_default();
        Ok::<_, std::io::Error>(FunctionOutput::with_result([(
            "out".to_string(),
            v.to_uppercase(),
        )]))
    })
    .unwrap();
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
    .unwrap();
    reg.register("explode", |_| async move {
        Err::<FunctionOutput, _>(std::io::Error::other("kaboom"))
    })
    .unwrap();
    FunctionActivities::new(reg)
}

async fn build_function_worker(tq: &str) -> altair_temporal::Worker {
    let temporal = temporal().await;
    let cfg = temporal.config(tq);
    WorkerBuilder::new(&cfg)
        .register_workflow::<FunctionPipelineWf>()
        .register_activities(make_function_activities())
        .build()
        .await
        .expect("build worker")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn function_pipeline_dispatches_registered_handlers_by_name() {
    let tq = unique("wf-fn-pipeline");
    let worker = build_function_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("fn-pipeline-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let tasks = vec![
            FunctionExecutionInput::new("upper").with_args([("text", "hello")]),
            FunctionExecutionInput::new("repeat").with_args([("text", "ab"), ("count", "3")]),
        ];
        let input = PipelineInput {
            tasks,
            stop_on_error: false,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                FunctionPipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: PipelineOutput<FunctionExecutionOutput> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.total_success, 2);
    assert_eq!(out.total_failed, 0);
    assert_eq!(
        out.results[0].result.get("out").map(String::as_str),
        Some("HELLO")
    );
    assert_eq!(
        out.results[1].result.get("out").map(String::as_str),
        Some("ababab")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn function_pipeline_continue_on_error_records_handler_failure_in_results() {
    let tq = unique("wf-fn-error");
    let worker = build_function_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("fn-error-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let tasks = vec![
            FunctionExecutionInput::new("upper").with_args([("text", "ok")]),
            FunctionExecutionInput::new("explode"),
            FunctionExecutionInput::new("upper").with_args([("text", "still ran")]),
        ];
        let input = PipelineInput {
            tasks,
            stop_on_error: false,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                FunctionPipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: PipelineOutput<FunctionExecutionOutput> =
        run_with_workload(worker, workload, Duration::from_secs(60)).await;
    // Handler errors are reported as unsuccessful outputs, not as
    // workflow failures (the activity still returns Ok).
    assert_eq!(out.total_success, 2);
    assert_eq!(out.total_failed, 1);
    assert!(out.results[1].error.contains("kaboom"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn function_unknown_handler_fails_with_activity_error() {
    let tq = unique("wf-fn-unknown");
    let worker = build_function_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("fn-unknown-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let input = PipelineInput {
            tasks: vec![FunctionExecutionInput::new("ghost")],
            stop_on_error: true,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                FunctionPipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        let res: Result<PipelineOutput<FunctionExecutionOutput>, _> =
            handle.get_result(WorkflowGetResultOptions::default()).await;
        res
    };

    let res = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    // Registry-miss is an infrastructure error → activity error →
    // workflow failure under stop_on_error.
    assert!(
        res.is_err(),
        "expected workflow failure for unknown handler",
    );
}

#[workflow]
#[derive(Default)]
pub struct FunctionParallelWf;

#[workflow_methods]
impl FunctionParallelWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: ParallelInput<FunctionExecutionInput>,
    ) -> WorkflowResult<ParallelOutput<FunctionExecutionOutput>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let result = parallel(input, |step| {
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

async fn build_function_parallel_worker(tq: &str) -> altair_temporal::Worker {
    let temporal = temporal().await;
    let cfg = temporal.config(tq);
    WorkerBuilder::new(&cfg)
        .register_workflow::<FunctionParallelWf>()
        .register_activities(make_function_activities())
        .build()
        .await
        .expect("build worker")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn function_pipeline_stop_on_error_fails_workflow_on_handler_unsuccessful_output() {
    // Pins down the cross-feature contract: handler errors come back
    // as `Ok(FunctionExecutionOutput { success: false, ... })`, but
    // the pipeline pattern inspects `TaskOutput::is_success()`. With
    // `stop_on_error: true`, an unsuccessful handler output therefore
    // raises `Error::PatternStopped`, surfacing as a workflow failure
    // (not a partial-result return).
    let tq = unique("wf-fn-stop");
    let worker = build_function_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("fn-stop-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let tasks = vec![
            FunctionExecutionInput::new("upper").with_args([("text", "first")]),
            FunctionExecutionInput::new("explode"),
            FunctionExecutionInput::new("upper").with_args([("text", "never-runs")]),
        ];
        let input = PipelineInput {
            tasks,
            stop_on_error: true,
            cleanup: false,
        };
        let handle = client
            .start_workflow(
                FunctionPipelineWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        let res: Result<PipelineOutput<FunctionExecutionOutput>, _> =
            handle.get_result(WorkflowGetResultOptions::default()).await;
        res
    };

    let res = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    let err = res.expect_err("expected workflow failure when handler reports unsuccessful output");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("kaboom"),
        "expected PatternStopped to carry handler error: {msg}",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn function_parallel_fail_fast_fails_workflow_on_handler_unsuccessful_output() {
    // Same contract for the parallel pattern: an unsuccessful handler
    // output under FailFast raises `PatternStopped` and the workflow
    // fails. (Under FailureStrategy::Continue the failure would be
    // recorded in `results` instead — that path is covered by
    // function_handler_error_becomes_unsuccessful_output.)
    let tq = unique("wf-fn-parallel-ff");
    let worker = build_function_parallel_worker(&tq).await;
    let client = temporal_client().await;
    let wf_id = unique("fn-parallel-ff-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let tasks = vec![
            FunctionExecutionInput::new("upper").with_args([("text", "a")]),
            FunctionExecutionInput::new("explode"),
            FunctionExecutionInput::new("upper").with_args([("text", "c")]),
        ];
        let input = ParallelInput {
            tasks,
            failure_strategy: FailureStrategy::FailFast,
        };
        let handle = client
            .start_workflow(
                FunctionParallelWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        let res: Result<ParallelOutput<FunctionExecutionOutput>, _> =
            handle.get_result(WorkflowGetResultOptions::default()).await;
        res
    };

    let res = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    let err = res.expect_err("expected workflow failure under FailFast + handler error");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("kaboom"),
        "expected PatternStopped to carry handler error: {msg}",
    );
}

// ---------------------------------------------------------------------------
// Datasync module (Phase 3): chunked_sync_run helper driving a real
// Temporal workflow with list-partitions / run-partition activities,
// plus optional cursor + continue-as-new.
// ---------------------------------------------------------------------------

use altair_wf::datasync::chunk::{
    ChunkedSyncConfig, Cursor, Partition, PartitionResult, SyncResult, chunked_sync_run,
};

// In-memory shared state for the datasync activities. Captures the
// partition list, the records each partition would yield, the cursor,
// and a per-partition fetch counter so assertions can verify dispatch.
#[derive(Default)]
pub struct DatasyncState {
    pub partitions: Vec<Partition<i64>>,
    pub records_per_partition: HashMap<i64, usize>,
    pub cursor: std::sync::Mutex<Option<i64>>,
    pub fetch_calls: std::sync::Mutex<Vec<i64>>,
    pub advance_calls: std::sync::Mutex<Vec<i64>>,
}

use std::collections::HashMap;
use std::sync::Arc;

pub struct DatasyncActivities {
    pub state: Arc<DatasyncState>,
}

#[activities]
impl DatasyncActivities {
    #[activity]
    pub async fn list_partitions(
        self: Arc<Self>,
        _ctx: ActivityContext,
    ) -> std::result::Result<Vec<Partition<i64>>, ActivityError> {
        Ok(self.state.partitions.clone())
    }

    #[activity]
    pub async fn run_partition(
        self: Arc<Self>,
        _ctx: ActivityContext,
        p: Partition<i64>,
    ) -> std::result::Result<PartitionResult<i64>, ActivityError> {
        self.state.fetch_calls.lock().unwrap().push(p.start);
        let n = self
            .state
            .records_per_partition
            .get(&p.start)
            .copied()
            .unwrap_or(0);
        Ok(PartitionResult {
            start: p.start,
            end: p.end,
            fetched: n,
            inserted: n,
            updated: 0,
            skipped: 0,
        })
    }

    #[activity]
    pub async fn read_cursor(
        self: Arc<Self>,
        _ctx: ActivityContext,
        _job: String,
    ) -> std::result::Result<Option<i64>, ActivityError> {
        Ok(*self.state.cursor.lock().unwrap())
    }

    #[activity]
    pub async fn advance_cursor(
        self: Arc<Self>,
        _ctx: ActivityContext,
        end: i64,
    ) -> std::result::Result<(), ActivityError> {
        self.state.advance_calls.lock().unwrap().push(end);
        *self.state.cursor.lock().unwrap() = Some(end);
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatasyncInput {
    pub job: String,
    pub use_cursor: bool,
    pub max_per_exec: usize,
}
impl TaskInput for DatasyncInput {}

#[workflow]
#[derive(Default)]
pub struct DatasyncWf;

#[workflow_methods]
impl DatasyncWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: DatasyncInput,
    ) -> WorkflowResult<SyncResult<i64>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;

        let list_opts = opts.clone();
        let list = || {
            let list_opts = list_opts.clone();
            async move {
                ctx_ref
                    .start_activity(DatasyncActivities::list_partitions, (), list_opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("list_partitions", e))
            }
        };

        let run_opts = opts.clone();
        let run = move |p: Partition<i64>| {
            let run_opts = run_opts.clone();
            async move {
                ctx_ref
                    .start_activity(DatasyncActivities::run_partition, p, run_opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("run_partition", e))
            }
        };

        let job_name = input.job.clone();
        let read_opts = opts.clone();
        let adv_opts = opts.clone();
        let cursor = if input.use_cursor {
            Cursor::Some {
                read: {
                    let job_name = job_name.clone();
                    move || {
                        let read_opts = read_opts.clone();
                        let job_name = job_name.clone();
                        async move {
                            ctx_ref
                                .start_activity(
                                    DatasyncActivities::read_cursor,
                                    job_name,
                                    read_opts,
                                )
                                .await
                                .map_err(|e| altair_wf::Error::activity("read_cursor", e))
                        }
                    }
                },
                advance: move |end: i64| {
                    let adv_opts = adv_opts.clone();
                    async move {
                        ctx_ref
                            .start_activity(DatasyncActivities::advance_cursor, end, adv_opts)
                            .await
                            .map_err(|e| altair_wf::Error::activity("advance_cursor", e))
                    }
                },
            }
        } else {
            Cursor::None
        };

        let cfg =
            ChunkedSyncConfig::new(&input.job).max_partitions_per_execution(input.max_per_exec);

        let result = chunked_sync_run(cfg, list, run, cursor, |_d| async {})
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

async fn build_datasync_worker(tq: &str, state: Arc<DatasyncState>) -> altair_temporal::Worker {
    let temporal = temporal().await;
    let cfg = temporal.config(tq);
    WorkerBuilder::new(&cfg)
        .register_workflow::<DatasyncWf>()
        .register_activities(DatasyncActivities { state })
        .build()
        .await
        .expect("build worker")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn datasync_chunked_run_without_cursor_visits_every_partition() {
    let mut records = HashMap::new();
    records.insert(0_i64, 5);
    records.insert(10_i64, 3);
    records.insert(20_i64, 7);
    let state = Arc::new(DatasyncState {
        partitions: vec![
            Partition::new(0_i64, 10),
            Partition::new(10, 20),
            Partition::new(20, 30),
        ],
        records_per_partition: records,
        ..DatasyncState::default()
    });

    let tq = unique("wf-ds-no-cursor");
    let worker = build_datasync_worker(&tq, state.clone()).await;
    let client = temporal_client().await;
    let wf_id = unique("ds-no-cursor-wid");
    let tq_clone = tq.clone();
    let cap_state = state.clone();

    let workload = async move {
        let input = DatasyncInput {
            job: "ds-1".into(),
            use_cursor: false,
            max_per_exec: 0,
        };
        let handle = client
            .start_workflow(
                DatasyncWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: SyncResult<i64> = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.job_name, "ds-1");
    assert_eq!(out.total_partitions, 3);
    assert_eq!(out.total_fetched, 5 + 3 + 7);
    assert_eq!(out.total_inserted, 5 + 3 + 7);
    assert!(!out.deferred);
    assert_eq!(*cap_state.fetch_calls.lock().unwrap(), vec![0, 10, 20],);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn datasync_chunked_run_with_cursor_skips_processed_partitions() {
    let mut records = HashMap::new();
    records.insert(0_i64, 1);
    records.insert(10_i64, 2);
    records.insert(20_i64, 3);
    let state = Arc::new(DatasyncState {
        partitions: vec![
            Partition::new(0_i64, 10),
            Partition::new(10, 20),
            Partition::new(20, 30),
        ],
        records_per_partition: records,
        cursor: std::sync::Mutex::new(Some(15_i64)),
        ..DatasyncState::default()
    });

    let tq = unique("wf-ds-cursor");
    let worker = build_datasync_worker(&tq, state.clone()).await;
    let client = temporal_client().await;
    let wf_id = unique("ds-cursor-wid");
    let tq_clone = tq.clone();
    let cap_state = state.clone();

    let workload = async move {
        let input = DatasyncInput {
            job: "ds-2".into(),
            use_cursor: true,
            max_per_exec: 0,
        };
        let handle = client
            .start_workflow(
                DatasyncWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: SyncResult<i64> = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    // cursor=15 keeps only start>=15, i.e. [20, 30) — 1 partition, 3 records.
    assert_eq!(out.total_partitions, 1);
    assert_eq!(out.total_fetched, 3);
    assert_eq!(out.partitions[0].start, 20);
    // advance_cursor recorded for the one processed partition.
    assert_eq!(*cap_state.advance_calls.lock().unwrap(), vec![30]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn datasync_chunked_run_truncates_to_max_and_sets_deferred() {
    let records: HashMap<i64, usize> = (0..10).map(|i| (i * 10, 1usize)).collect();
    let state = Arc::new(DatasyncState {
        partitions: (0..10)
            .map(|i| Partition::new(i * 10, (i + 1) * 10))
            .collect(),
        records_per_partition: records,
        ..DatasyncState::default()
    });

    let tq = unique("wf-ds-max");
    let worker = build_datasync_worker(&tq, state.clone()).await;
    let client = temporal_client().await;
    let wf_id = unique("ds-max-wid");
    let tq_clone = tq.clone();
    let cap_state = state.clone();

    let workload = async move {
        let input = DatasyncInput {
            job: "ds-3".into(),
            use_cursor: true,
            max_per_exec: 3,
        };
        let handle = client
            .start_workflow(
                DatasyncWf::run,
                input,
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let out: SyncResult<i64> = run_with_workload(worker, workload, Duration::from_secs(60)).await;
    assert_eq!(out.total_partitions, 3);
    assert!(out.deferred, "expected deferred=true when truncated");
    // Advance recorded for each of the 3 processed partitions, capturing
    // their `end` values: 10, 20, 30.
    assert_eq!(*cap_state.advance_calls.lock().unwrap(), vec![10, 20, 30],);
}
