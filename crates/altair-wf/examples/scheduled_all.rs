//! `altair-wf` — schedule **every** example workflow against a live
//! Temporal dev server. One worker, several `Schedule`s, with each
//! pattern + feature represented by 2-3 variations so the Temporal UI
//! shows the whole feature surface in motion.
//!
//! Pairs naturally with `temporal server start-dev`:
//!
//! ```bash
//! # Terminal 1: start the dev server (UI at http://localhost:8233)
//! temporal server start-dev
//!
//! # Terminal 2: build, run, and let it sit. Each schedule fires every
//! # 2-3 minutes; the trigger sub-command (or temporal schedule
//! # trigger) gives you an instant first run.
//! cargo run -p altair-wf --features 'function datasync' --example scheduled_all
//! ```
//!
//! Variations (every workflow takes `()` because the current
//! [`Schedule`] API doesn't accept arguments; each workflow constructs
//! its real input inline):
//!
//! - **execute**: `_ok` (single success), `_fail` (returns
//!   `is_success() == false`)
//! - **pipeline**: `_all_ok`, `_continue` (mixed success +
//!   `stop_on_error=false`), `_stop` (mixed +
//!   `stop_on_error=true`, surfaces as workflow failure)
//! - **parallel**: `_continue`, `_fail_fast`
//! - **run_loop**: `_sequential`, `_parallel`
//! - **parameterized_loop**: one variant (cartesian product, ordered)
//! - **run_dag**: `_diamond` (build/test/lint/deploy), `_linear`
//!   (a→b→c→d straight chain)
//! - **function**: `_success` (all handlers succeed), `_mixed`
//!   (one handler reports `success: false`)
//! - **chunked datasync**: `_can` (continue-as-new chain, 6
//!   partitions × 2 per execution), `_single` (3 partitions, fits
//!   one execution)
//!
//! Schedule ids are stable so re-running this example is safe (uses
//! [`Schedule::create_or_update`]). Stop with Ctrl-C; the worker
//! drains gracefully (see `Config::shutdown_grace`).
//!
//! Inspect:
//! - schedules: http://localhost:8233/namespaces/default/schedules
//! - workflows: http://localhost:8233/namespaces/default/workflows

#![allow(
    missing_docs,
    clippy::unused_async,
    clippy::too_many_lines,
    clippy::zero_sized_map_values,
    clippy::doc_markdown
)]

use std::sync::Arc;
use std::time::Duration;

use altair_temporal::temporalio_client::Client;
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    ContinueAsNewOptions, WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_temporal::{Schedule, WorkerBuilder};
use altair_wf::datasync::chunk::{
    ChunkedSyncConfig, ChunkedSyncSummary, Cursor, Partition, PartitionResult, chunked_sync_run,
};
use altair_wf::function::{
    FunctionActivities, FunctionExecutionInput, FunctionExecutionOutput, FunctionInput,
    FunctionOutput, Registry,
};
use altair_wf::{
    DAGInput, DAGNode, DAGOutput, FailureStrategy, LoopInput, LoopOutput, ParallelInput,
    ParallelOutput, ParameterizedLoopInput, PipelineInput, PipelineOutput, Substitutor, TaskInput,
    TaskOutput, default_activity_options, execute, parallel, parameterized_loop, pipeline, run_dag,
    run_loop, substitutor_from_fn,
};

// ---------------------------------------------------------------------------
// Echo payloads + activity, shared across every pattern variant
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
        let echoed = if input.will_fail {
            format!("simulated business failure for {}", input.msg)
        } else {
            format!("echo:{}", input.msg)
        };
        Ok(EchoOut {
            id: input.id,
            echoed,
            ok: !input.will_fail,
        })
    }
}

fn ok(id: u32, name: &str) -> EchoIn {
    EchoIn {
        id,
        msg: name.into(),
        will_fail: false,
    }
}
fn bad(id: u32, name: &str) -> EchoIn {
    EchoIn {
        id,
        msg: name.into(),
        will_fail: true,
    }
}

/// Boilerplate-shrinker: wrap `EchoActivities::echo` as a
/// closure for use inside `pipeline` / `parallel` / etc.
macro_rules! dispatch_echo {
    ($ctx:expr, $opts:expr) => {
        |step: EchoIn| {
            let opts = $opts.clone();
            async move {
                $ctx.start_activity(EchoActivities::echo, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("EchoActivities::echo", e))
            }
        }
    };
}

// ---------------------------------------------------------------------------
// execute (single task) variants
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledExecuteOkWf;

#[workflow_methods]
impl ScheduledExecuteOkWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<EchoOut> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let out = execute(ok(1, "single-success"), dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(out)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledExecuteFailWf;

#[workflow_methods]
impl ScheduledExecuteFailWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<EchoOut> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // is_success() returns false; execute still returns Ok because
        // the activity itself succeeded — the workflow body decides
        // what to do with the business-level failure.
        let out = execute(bad(2, "single-failure"), dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// pipeline variants
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledPipelineAllOkWf;

#[workflow_methods]
impl ScheduledPipelineAllOkWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<PipelineOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = PipelineInput {
            tasks: vec![ok(1, "alice"), ok(2, "bob"), ok(3, "carol")],
            stop_on_error: false,
            cleanup: false,
        };
        let result = pipeline(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledPipelineContinueWf;

#[workflow_methods]
impl ScheduledPipelineContinueWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<PipelineOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // Middle task reports failure; `stop_on_error=false` keeps
        // going. Caller observes `total_failed=1` and the failing
        // index in `failed_indices`.
        let input = PipelineInput {
            tasks: vec![ok(10, "step-a"), bad(11, "step-b"), ok(12, "step-c")],
            stop_on_error: false,
            cleanup: false,
        };
        let result = pipeline(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledPipelineStopWf;

#[workflow_methods]
impl ScheduledPipelineStopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<PipelineOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // `stop_on_error=true` + failing middle task → the helper
        // returns `Error::PatternStopped` → workflow fails. The UI
        // shows this as "Failed" with the position + reason.
        let input = PipelineInput {
            tasks: vec![ok(20, "step-a"), bad(21, "step-b"), ok(22, "step-c")],
            stop_on_error: true,
            cleanup: false,
        };
        let result = pipeline(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// parallel variants
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledParallelContinueWf;

#[workflow_methods]
impl ScheduledParallelContinueWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<ParallelOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = ParallelInput {
            tasks: vec![
                ok(30, "url-a"),
                bad(31, "url-b"),
                ok(32, "url-c"),
                ok(33, "url-d"),
            ],
            failure_strategy: FailureStrategy::Continue,
            max_in_flight: 0,
        };
        let result = parallel(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledParallelFailFastWf;

#[workflow_methods]
impl ScheduledParallelFailFastWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<ParallelOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = ParallelInput {
            tasks: vec![ok(40, "url-a"), bad(41, "url-b"), ok(42, "url-c")],
            failure_strategy: FailureStrategy::FailFast,
            max_in_flight: 0,
        };
        // FailFast surfaces the first failure as a workflow failure.
        let result = parallel(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// run_loop variants
// ---------------------------------------------------------------------------

fn substitutor() -> Substitutor<EchoIn> {
    substitutor_from_fn(|template: &EchoIn, item: &str, idx: usize, _params| EchoIn {
        id: template.id + u32::try_from(idx).unwrap_or(0),
        msg: format!("{}-{item}", template.msg),
        will_fail: template.will_fail,
    })
}

#[workflow]
#[derive(Default)]
pub struct ScheduledLoopSequentialWf;

#[workflow_methods]
impl ScheduledLoopSequentialWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<LoopOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = LoopInput {
            items: vec!["us-east-1".into(), "eu-west-1".into(), "ap-southeast-1".into()],
            template: ok(50, "deploy"),
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
            max_in_flight: 0,
        };
        let result = run_loop(input, substitutor(), dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledLoopParallelWf;

#[workflow_methods]
impl ScheduledLoopParallelWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<LoopOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = LoopInput {
            items: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            template: ok(60, "fan-out"),
            parallel: true,
            failure_strategy: FailureStrategy::Continue,
            max_in_flight: 0,
        };
        let result = run_loop(input, substitutor(), dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// parameterized_loop
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledParameterizedLoopWf;

#[workflow_methods]
impl ScheduledParameterizedLoopWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<LoopOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // 2 regions × 2 tiers = 4 cartesian combinations.
        let mut parameters: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        parameters.insert(
            "region".into(),
            vec!["us-east-1".into(), "eu-west-1".into()],
        );
        parameters.insert("tier".into(), vec!["standard".into(), "premium".into()]);
        let input = ParameterizedLoopInput {
            parameters,
            template: ok(70, "deploy"),
            parallel: true,
            failure_strategy: FailureStrategy::Continue,
            max_in_flight: 0,
        };
        let sub: Substitutor<EchoIn> = substitutor_from_fn(
            |template: &EchoIn, _item: &str, idx: usize, params| EchoIn {
                id: template.id + u32::try_from(idx).unwrap_or(0),
                msg: format!(
                    "{}/{}/{}",
                    template.msg,
                    params.get("region").cloned().unwrap_or_default(),
                    params.get("tier").cloned().unwrap_or_default(),
                ),
                will_fail: false,
            },
        );
        let result = parameterized_loop(input, sub, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// run_dag variants
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledDagDiamondWf;

#[workflow_methods]
impl ScheduledDagDiamondWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<DAGOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = DAGInput {
            nodes: vec![
                DAGNode {
                    name: "build".into(),
                    input: ok(80, "build"),
                    dependencies: vec![],
                },
                DAGNode {
                    name: "test".into(),
                    input: ok(81, "test"),
                    dependencies: vec!["build".into()],
                },
                DAGNode {
                    name: "lint".into(),
                    input: ok(82, "lint"),
                    dependencies: vec!["build".into()],
                },
                DAGNode {
                    name: "deploy".into(),
                    input: ok(83, "deploy"),
                    dependencies: vec!["test".into(), "lint".into()],
                },
            ],
            fail_fast: true,
            max_parallel: 0,
        };
        let result = run_dag(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledDagLinearWf;

#[workflow_methods]
impl ScheduledDagLinearWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<DAGOutput<EchoOut>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // Linear chain a → b → c → d.
        let input = DAGInput {
            nodes: vec![
                DAGNode {
                    name: "a".into(),
                    input: ok(90, "stage-a"),
                    dependencies: vec![],
                },
                DAGNode {
                    name: "b".into(),
                    input: ok(91, "stage-b"),
                    dependencies: vec!["a".into()],
                },
                DAGNode {
                    name: "c".into(),
                    input: ok(92, "stage-c"),
                    dependencies: vec!["b".into()],
                },
                DAGNode {
                    name: "d".into(),
                    input: ok(93, "stage-d"),
                    dependencies: vec!["c".into()],
                },
            ],
            fail_fast: true,
            max_parallel: 0,
        };
        let result = run_dag(input, dispatch_echo!(ctx_ref, opts))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// function feature variants
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledFunctionSuccessWf;

#[workflow_methods]
impl ScheduledFunctionSuccessWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<PipelineOutput<FunctionExecutionOutput>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        let input = PipelineInput {
            tasks: vec![
                FunctionExecutionInput::new("upper").with_args([("text", "hello")]),
                FunctionExecutionInput::new("reverse").with_args([("text", "altair-wf")]),
                FunctionExecutionInput::new("repeat").with_args([("text", "ab"), ("count", "3")]),
            ],
            stop_on_error: false,
            cleanup: false,
        };
        let result = pipeline(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(FunctionActivities::execute_function, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("FunctionActivities", e))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

#[workflow]
#[derive(Default)]
pub struct ScheduledFunctionMixedWf;

#[workflow_methods]
impl ScheduledFunctionMixedWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<PipelineOutput<FunctionExecutionOutput>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // Middle handler returns Err → execute_function reports
        // `success: false` on the activity output (NOT a workflow
        // failure, since handler errors aren't infrastructure
        // errors). Caller sees `total_failed=1`.
        let input = PipelineInput {
            tasks: vec![
                FunctionExecutionInput::new("upper").with_args([("text", "ok")]),
                FunctionExecutionInput::new("explode"),
                FunctionExecutionInput::new("upper").with_args([("text", "still-runs")]),
            ],
            stop_on_error: false,
            cleanup: false,
        };
        let result = pipeline(input, |step| {
            let opts = opts.clone();
            async move {
                ctx_ref
                    .start_activity(FunctionActivities::execute_function, step, opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("FunctionActivities", e))
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
    reg.register("reverse", |input: FunctionInput| async move {
        let text = input.args.get("text").cloned().unwrap_or_default();
        Ok::<_, std::io::Error>(FunctionOutput::with_result([(
            "out".to_string(),
            text.chars().rev().collect::<String>(),
        )]))
    })
    .expect("register reverse");
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
    reg.register("explode", |_input: FunctionInput| async {
        Err::<FunctionOutput, _>(std::io::Error::other("intentional handler failure"))
    })
    .expect("register explode");
    reg
}

// ---------------------------------------------------------------------------
// chunked datasync variants
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct DemoState {
    pub partitions: Vec<Partition<i64>>,
    /// **Per-job** cursor — each job (e.g. `"scheduled-can"`,
    /// `"scheduled-single"`) has its own entry so the two chunked
    /// schedules don't interfere with each other.
    pub cursors: std::sync::Mutex<std::collections::HashMap<String, i64>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdvanceInput {
    pub job: String,
    pub end: i64,
}

pub struct DemoActivities {
    pub state: Arc<DemoState>,
}

#[activities]
impl DemoActivities {
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
        Ok(PartitionResult {
            start: p.start,
            end: p.end,
            fetched: 3,
            inserted: 3,
            updated: 0,
            skipped: 0,
        })
    }

    #[activity]
    pub async fn read_cursor(
        self: Arc<Self>,
        _ctx: ActivityContext,
        job: String,
    ) -> std::result::Result<Option<i64>, ActivityError> {
        Ok(self.state.cursors.lock().unwrap().get(&job).copied())
    }

    #[activity]
    pub async fn advance_cursor(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: AdvanceInput,
    ) -> std::result::Result<(), ActivityError> {
        self.state
            .cursors
            .lock()
            .unwrap()
            .insert(input.job, input.end);
        Ok(())
    }

    /// Drop a job's cursor — used by the workflows at top-level start
    /// (i.e. when they were NOT spawned via continue-as-new) so each
    /// schedule fire processes every partition again.
    #[activity]
    pub async fn reset_cursor(
        self: Arc<Self>,
        _ctx: ActivityContext,
        job: String,
    ) -> std::result::Result<(), ActivityError> {
        self.state.cursors.lock().unwrap().remove(&job);
        Ok(())
    }
}

// Each chunked variant builds its own helper closures inline. Repeated
// to keep each workflow self-contained in the file (and so each fires
// its own continue-as-new chain that's visible in the UI).
macro_rules! chunked_run {
    ($ctx:expr, $opts:expr, $job:expr, $max:expr) => {{
        let job_name: String = $job.into();
        let max_per_exec: usize = $max;
        let opts = $opts;
        let ctx_ref = $ctx;
        let list_opts = opts.clone();
        let list = || {
            let list_opts = list_opts.clone();
            async move {
                ctx_ref
                    .start_activity(DemoActivities::list_partitions, (), list_opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("list_partitions", e))
            }
        };
        let run_opts = opts.clone();
        let run = move |p: Partition<i64>| {
            let run_opts = run_opts.clone();
            async move {
                ctx_ref
                    .start_activity(DemoActivities::run_partition, p, run_opts)
                    .await
                    .map_err(|e| altair_wf::Error::activity("run_partition", e))
            }
        };
        let read_opts = opts.clone();
        let adv_opts = opts.clone();
        let job_for_read = job_name.clone();
        let job_for_advance = job_name.clone();
        let cursor = Cursor::Some {
            read: move || {
                let read_opts = read_opts.clone();
                let job_for_read = job_for_read.clone();
                async move {
                    ctx_ref
                        .start_activity(DemoActivities::read_cursor, job_for_read, read_opts)
                        .await
                        .map_err(|e| altair_wf::Error::activity("read_cursor", e))
                }
            },
            advance: move |end: i64| {
                let adv_opts = adv_opts.clone();
                let job = job_for_advance.clone();
                async move {
                    ctx_ref
                        .start_activity(
                            DemoActivities::advance_cursor,
                            AdvanceInput { job, end },
                            adv_opts,
                        )
                        .await
                        .map_err(|e| altair_wf::Error::activity("advance_cursor", e))
                }
            },
        };
        let cfg = ChunkedSyncConfig::new(&job_name).max_partitions_per_execution(max_per_exec);
        chunked_sync_run(cfg, list, run, cursor, |_d| async {}).await
    }};
}

/// Continue-as-new chain — 6 partitions × max-per-exec 2 = three
/// chained workflow executions per schedule fire.
#[workflow]
#[derive(Default)]
pub struct ScheduledChunkedCanWf;

#[workflow_methods]
impl ScheduledChunkedCanWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<ChunkedSyncSummary<i64>> {
        let opts = default_artifact_opts();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // Top-level start (the schedule fired) — wipe the cursor so we
        // process every partition again. A continued execution inside
        // the CAN chain skips this so it can read the prior cursor.
        if ctx_ref
            .workflow_initial_info()
            .continued_from_execution_run_id
            .is_empty()
        {
            ctx_ref
                .start_activity(DemoActivities::reset_cursor, "scheduled-can".to_string(), opts.clone())
                .await
                .map_err(|e| anyhow::anyhow!("reset_cursor: {e}"))?;
        }
        let result = chunked_run!(ctx_ref, opts, "scheduled-can", 2)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if result.deferred {
            ctx_ref.continue_as_new(&(), ContinueAsNewOptions::default())?;
            unreachable!();
        }
        Ok(result)
    }
}

/// All partitions fit in one execution — no continue-as-new fires,
/// useful contrast in the UI history.
#[workflow]
#[derive(Default)]
pub struct ScheduledChunkedSinglePassWf;

#[workflow_methods]
impl ScheduledChunkedSinglePassWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        _input: (),
    ) -> WorkflowResult<ChunkedSyncSummary<i64>> {
        let opts = default_artifact_opts();
        let ctx_ref: &WorkflowContext<Self> = ctx;
        // This variant never continues-as-new; every fire starts fresh.
        ctx_ref
            .start_activity(DemoActivities::reset_cursor, "scheduled-single".to_string(), opts.clone())
            .await
            .map_err(|e| anyhow::anyhow!("reset_cursor: {e}"))?;
        // max_per_exec = 0 disables truncation; entire list runs once.
        let result = chunked_run!(ctx_ref, opts, "scheduled-single", 0)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(result)
    }
}

/// Alias kept so the `chunked_run!` macro's `opts.clone()` borrowing
/// works inside both chunked workflow variants without renaming.
fn default_artifact_opts() -> altair_temporal::temporalio_sdk::ActivityOptions {
    default_activity_options()
}

// ---------------------------------------------------------------------------
// Schedule registration helpers
// ---------------------------------------------------------------------------

fn schedule_id(suffix: &str) -> String {
    format!("altair-wf-scheduled-{suffix}")
}
fn workflow_id(suffix: &str) -> String {
    format!("altair-wf-{suffix}-recurring")
}

/// `(workflow_type, schedule_suffix, interval, note)`.
const SCHEDULE_PLAN: &[(&str, &str, u64, &str)] = &[
    // execute
    ("ScheduledExecuteOkWf",            "execute-ok",        2, "execute pattern, single task success"),
    ("ScheduledExecuteFailWf",          "execute-fail",      3, "execute pattern, business-logic failure (is_success=false)"),
    // pipeline
    ("ScheduledPipelineAllOkWf",        "pipeline-all-ok",   2, "pipeline, all steps succeed"),
    ("ScheduledPipelineContinueWf",     "pipeline-continue", 3, "pipeline, mixed success with stop_on_error=false"),
    ("ScheduledPipelineStopWf",         "pipeline-stop",     2, "pipeline, stop_on_error=true → workflow fails on mid failure"),
    // parallel
    ("ScheduledParallelContinueWf",     "parallel-continue", 3, "parallel, Continue strategy collects every outcome"),
    ("ScheduledParallelFailFastWf",     "parallel-fail-fast",2, "parallel, FailFast → workflow fails on first failure"),
    // loop
    ("ScheduledLoopSequentialWf",       "loop-sequential",   3, "run_loop sequential per-item"),
    ("ScheduledLoopParallelWf",         "loop-parallel",     2, "run_loop parallel per-item"),
    // parameterized loop
    ("ScheduledParameterizedLoopWf",    "parameterized-loop",3, "parameterized_loop cartesian product"),
    // DAG
    ("ScheduledDagDiamondWf",           "dag-diamond",       2, "run_dag, diamond (build/test/lint/deploy)"),
    ("ScheduledDagLinearWf",            "dag-linear",        3, "run_dag, linear chain a→b→c→d"),
    // function feature
    ("ScheduledFunctionSuccessWf",      "function-success",  2, "function: registry pipeline, all handlers succeed"),
    ("ScheduledFunctionMixedWf",        "function-mixed",    3, "function: middle handler reports success=false"),
    // chunked datasync
    ("ScheduledChunkedCanWf",           "chunked-can",       3, "datasync chunk + continue-as-new (3 executions per fire)"),
    ("ScheduledChunkedSinglePassWf",    "chunked-single",    2, "datasync chunk, all partitions in one execution"),
];

async fn register_schedules(client: &Client, task_queue: &str) -> anyhow::Result<()> {
    for &(wf_type, suffix, mins, note) in SCHEDULE_PLAN {
        Schedule::builder()
            .interval(Duration::from_mins(mins))
            .note(note)
            .start_workflow(wf_type, task_queue, workflow_id(suffix))
            .create_or_update(client, schedule_id(suffix))
            .await?;
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,altair_wf=info,altair_temporal=info".into()),
        )
        .init();

    let cfg = altair_temporal::Config {
        task_queue: "altair-wf-scheduled".to_string(),
        ..Default::default()
    };

    let chunked_state = Arc::new(DemoState {
        partitions: (0..6).map(|i| Partition::new(i * 10, (i + 1) * 10)).collect(),
        ..DemoState::default()
    });

    let worker = WorkerBuilder::new(&cfg)
        // patterns
        .register_workflow::<ScheduledExecuteOkWf>()
        .register_workflow::<ScheduledExecuteFailWf>()
        .register_workflow::<ScheduledPipelineAllOkWf>()
        .register_workflow::<ScheduledPipelineContinueWf>()
        .register_workflow::<ScheduledPipelineStopWf>()
        .register_workflow::<ScheduledParallelContinueWf>()
        .register_workflow::<ScheduledParallelFailFastWf>()
        .register_workflow::<ScheduledLoopSequentialWf>()
        .register_workflow::<ScheduledLoopParallelWf>()
        .register_workflow::<ScheduledParameterizedLoopWf>()
        .register_workflow::<ScheduledDagDiamondWf>()
        .register_workflow::<ScheduledDagLinearWf>()
        // function feature
        .register_workflow::<ScheduledFunctionSuccessWf>()
        .register_workflow::<ScheduledFunctionMixedWf>()
        // chunked datasync
        .register_workflow::<ScheduledChunkedCanWf>()
        .register_workflow::<ScheduledChunkedSinglePassWf>()
        // activities (shared across many workflows)
        .register_activities(EchoActivities)
        .register_activities(FunctionActivities::new(build_registry()))
        .register_activities(DemoActivities {
            state: chunked_state,
        })
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    register_schedules(&client, &cfg.task_queue).await?;

    println!();
    println!(
        "registered {} schedules on task queue {}:",
        SCHEDULE_PLAN.len(),
        cfg.task_queue,
    );
    for &(_, suffix, mins, _) in SCHEDULE_PLAN {
        println!("  - {} (every {}m)", schedule_id(suffix), mins);
    }
    println!();
    println!("Temporal UI:");
    println!("  schedules: http://localhost:8233/namespaces/default/schedules");
    println!("  workflows: http://localhost:8233/namespaces/default/workflows");
    println!();
    println!("worker running — Ctrl-C to stop (graceful drain on shutdown).");
    println!();

    worker.run().await?;
    Ok(())
}
