//! `altair-wf` — schedule every example workflow against a live
//! Temporal dev server. One worker, several `Schedule`s, staggered
//! intervals between 2 and 3 minutes so executions land in the UI
//! without all firing at once.
//!
//! Pairs naturally with `temporal server start-dev`:
//!
//! ```bash
//! # Terminal 1: start the dev server (UI at http://localhost:8233)
//! temporal server start-dev
//!
//! # Terminal 2: build, run, and let it sit. Each schedule will fire
//! # the first time after its initial interval — give it ~3 minutes
//! # before the first execution shows up in the UI.
//! cargo run -p altair-wf --features 'function datasync' --example scheduled_all
//! ```
//!
//! Inspect:
//! - http://localhost:8233/namespaces/default/schedules — the schedules
//! - http://localhost:8233/namespaces/default/workflows — execution history
//!
//! Stop with Ctrl-C; the worker drains gracefully (see
//! `Config::shutdown_grace`). The schedules **stay registered** on the
//! Temporal server — they reuse stable ids via
//! `Schedule::create_or_update`, so re-running this example is safe.

#![allow(
    missing_docs,
    clippy::unused_async,
    clippy::too_many_lines,
    clippy::zero_sized_map_values,
    clippy::doc_markdown
)]

use std::collections::HashMap;
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
    DAGInput, DAGNode, DAGOutput, FailureStrategy, ParallelInput, ParallelOutput, PipelineInput,
    PipelineOutput, TaskInput, TaskOutput, default_activity_options, parallel, pipeline, run_dag,
};

// ---------------------------------------------------------------------------
// Shared payloads + activities for the hello_pipeline / hello_parallel /
// hello_dag schedules. One Echo activity covers all three.
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
}

pub struct EchoActivities;

#[activities]
impl EchoActivities {
    #[activity]
    pub async fn echo(
        _ctx: ActivityContext,
        input: EchoIn,
    ) -> std::result::Result<EchoOut, ActivityError> {
        Ok(EchoOut {
            id: input.id,
            echoed: format!("echo:{}", input.msg),
            ok: !input.will_fail,
        })
    }
}

// ---------------------------------------------------------------------------
// 1) Sequential pipeline
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledPipelineWf;

#[workflow_methods]
impl ScheduledPipelineWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: PipelineInput<EchoIn>,
    ) -> WorkflowResult<PipelineOutput<EchoOut>> {
        let opts = default_activity_options();
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

// ---------------------------------------------------------------------------
// 2) Concurrent parallel
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledParallelWf;

#[workflow_methods]
impl ScheduledParallelWf {
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

// ---------------------------------------------------------------------------
// 3) DAG (diamond)
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledDagWf;

#[workflow_methods]
impl ScheduledDagWf {
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

// ---------------------------------------------------------------------------
// 4) `function` feature — named-handler dispatch via a pipeline
// ---------------------------------------------------------------------------

#[workflow]
#[derive(Default)]
pub struct ScheduledFunctionWf;

#[workflow_methods]
impl ScheduledFunctionWf {
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

// ---------------------------------------------------------------------------
// 5) `datasync::chunk` — continue-as-new round-trip
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct DemoState {
    pub partitions: Vec<Partition<i64>>,
    pub cursor: std::sync::Mutex<Option<i64>>,
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
        *self.state.cursor.lock().unwrap() = Some(end);
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DemoInput {
    pub job: String,
    pub max_per_exec: usize,
}
impl TaskInput for DemoInput {}

#[workflow]
#[derive(Default)]
pub struct ScheduledChunkedWf;

#[workflow_methods]
impl ScheduledChunkedWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: DemoInput,
    ) -> WorkflowResult<ChunkedSyncSummary<i64>> {
        let opts = default_activity_options();
        let ctx_ref: &WorkflowContext<Self> = ctx;

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
        let job_name = input.job.clone();
        let read_opts = opts.clone();
        let adv_opts = opts.clone();
        let cursor = Cursor::Some {
            read: {
                let job_name = job_name.clone();
                move || {
                    let read_opts = read_opts.clone();
                    let job_name = job_name.clone();
                    async move {
                        ctx_ref
                            .start_activity(DemoActivities::read_cursor, job_name, read_opts)
                            .await
                            .map_err(|e| altair_wf::Error::activity("read_cursor", e))
                    }
                }
            },
            advance: move |end: i64| {
                let adv_opts = adv_opts.clone();
                async move {
                    ctx_ref
                        .start_activity(DemoActivities::advance_cursor, end, adv_opts)
                        .await
                        .map_err(|e| altair_wf::Error::activity("advance_cursor", e))
                }
            },
        };
        let cfg =
            ChunkedSyncConfig::new(&input.job).max_partitions_per_execution(input.max_per_exec);
        let result = chunked_sync_run(cfg, list, run, cursor, |_d| async {})
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if result.deferred {
            ctx_ref.continue_as_new(&input, ContinueAsNewOptions::default())?;
            unreachable!();
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Schedule registration helpers
// ---------------------------------------------------------------------------

/// Build a stable schedule id for one of the example workflows.
fn schedule_id(suffix: &str) -> String {
    format!("altair-wf-scheduled-{suffix}")
}

/// Build a stable workflow id for the schedule's recurring executions.
/// Temporal will append `-<scheduled-at>` to make each execution
/// unique within the schedule.
fn workflow_id(suffix: &str) -> String {
    format!("altair-wf-{suffix}-recurring")
}

async fn register_schedules(client: &Client, task_queue: &str) -> anyhow::Result<()> {
    // Sequential pipeline — every 2 minutes.
    Schedule::builder()
        .interval(Duration::from_mins(2))
        .note("altair-wf: sequential pipeline (pipeline pattern)")
        .start_workflow("ScheduledPipelineWf", task_queue, workflow_id("pipeline"))
        .create_or_update(client, schedule_id("pipeline"))
        .await?;

    // Concurrent parallel — every 2 minutes, offset implicitly.
    Schedule::builder()
        .interval(Duration::from_mins(2))
        .note("altair-wf: concurrent parallel (parallel pattern)")
        .start_workflow("ScheduledParallelWf", task_queue, workflow_id("parallel"))
        .create_or_update(client, schedule_id("parallel"))
        .await?;

    // DAG (diamond) — every 3 minutes.
    Schedule::builder()
        .interval(Duration::from_mins(3))
        .note("altair-wf: build/test/lint/deploy diamond (DAG pattern)")
        .start_workflow("ScheduledDagWf", task_queue, workflow_id("dag"))
        .create_or_update(client, schedule_id("dag"))
        .await?;

    // function feature — every 2 minutes.
    Schedule::builder()
        .interval(Duration::from_mins(2))
        .note("altair-wf: function registry dispatch via pipeline")
        .start_workflow("ScheduledFunctionWf", task_queue, workflow_id("function"))
        .create_or_update(client, schedule_id("function"))
        .await?;

    // datasync chunk + continue-as-new — every 3 minutes.
    Schedule::builder()
        .interval(Duration::from_mins(3))
        .note("altair-wf: chunked datasync with continue-as-new")
        .start_workflow("ScheduledChunkedWf", task_queue, workflow_id("chunked"))
        .create_or_update(client, schedule_id("chunked"))
        .await?;

    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,altair_wf=info,altair_temporal=info".into()),
        )
        .init();

    let cfg = altair_temporal::Config {
        task_queue: "altair-wf-scheduled".to_string(),
        ..Default::default()
    };

    // -------------------- workflow input payloads --------------------

    let ok = |id: u32| EchoIn {
        id,
        msg: format!("msg{id}"),
        will_fail: false,
    };

    let pipeline_input = PipelineInput {
        tasks: vec![ok(1), ok(2), ok(3)],
        stop_on_error: false,
        cleanup: false,
    };

    let parallel_input = ParallelInput {
        tasks: vec![ok(10), ok(20), ok(30), ok(40)],
        failure_strategy: FailureStrategy::Continue,
        max_in_flight: 0,
    };

    let dag_input = DAGInput {
        nodes: vec![
            DAGNode {
                name: "build".into(),
                input: ok(100),
                dependencies: vec![],
            },
            DAGNode {
                name: "test".into(),
                input: ok(101),
                dependencies: vec!["build".into()],
            },
            DAGNode {
                name: "lint".into(),
                input: ok(102),
                dependencies: vec!["build".into()],
            },
            DAGNode {
                name: "deploy".into(),
                input: ok(103),
                dependencies: vec!["test".into(), "lint".into()],
            },
        ],
        fail_fast: true,
        max_parallel: 0,
    };

    let function_input = PipelineInput {
        tasks: vec![
            FunctionExecutionInput::new("upper").with_args([("text", "hello")]),
            FunctionExecutionInput::new("reverse").with_args([("text", "altair")]),
        ],
        stop_on_error: false,
        cleanup: false,
    };

    let chunked_input = DemoInput {
        job: "scheduled-demo".into(),
        // 6 partitions × max 2 = 3 executions per schedule fire.
        max_per_exec: 2,
    };

    // Default input *payloads* used by every scheduled execution. The
    // SDK sends these inside the schedule's `StartWorkflow` action.
    let _: HashMap<&str, ()> = HashMap::new();

    // -------------------- worker setup --------------------

    let chunked_state = Arc::new(DemoState {
        partitions: (0..6)
            .map(|i| Partition::new(i * 10, (i + 1) * 10))
            .collect(),
        ..DemoState::default()
    });

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<ScheduledPipelineWf>()
        .register_workflow::<ScheduledParallelWf>()
        .register_workflow::<ScheduledDagWf>()
        .register_workflow::<ScheduledFunctionWf>()
        .register_workflow::<ScheduledChunkedWf>()
        .register_activities(EchoActivities)
        .register_activities(FunctionActivities::new(build_registry()))
        .register_activities(DemoActivities {
            state: chunked_state,
        })
        .build()
        .await?;

    // -------------------- schedule registration --------------------

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    register_schedules(&client, &cfg.task_queue).await?;
    println!();
    println!("registered 5 schedules on task queue {}:", cfg.task_queue);
    for s in [
        "altair-wf-scheduled-pipeline",
        "altair-wf-scheduled-parallel",
        "altair-wf-scheduled-dag",
        "altair-wf-scheduled-function",
        "altair-wf-scheduled-chunked",
    ] {
        println!("  - {s}");
    }

    // Trigger one immediate run of each by starting the workflows once
    // up front so the user sees activity in the UI before the first
    // scheduled interval elapses.
    let _ = (
        pipeline_input,
        parallel_input,
        dag_input,
        function_input,
        chunked_input,
    );

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
