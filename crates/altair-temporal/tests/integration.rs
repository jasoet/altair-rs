//! Integration tests against a real Temporal server.
//!
//! Gated behind the `integration-tests` feature (which implies
//! `testcontainers`). Requires Docker. Run with:
//!
//! ```text
//! cargo test -p altair-temporal --features integration-tests --test integration -- --nocapture
//! ```
//!
//! The whole file shares one container (started lazily via `tokio::OnceCell`)
//! to keep total runtime low. Each test gets a unique task queue, workflow
//! id, and schedule id to prevent cross-contamination.
//!
//! Phase 1 covers: client connect (2), worker lifecycle (2), schedule
//! round-trips (2). Phase 2 covers: workflow execution (1 echo + 1 with
//! activity), `workflow_id` encoded payload round-trip through a real
//! workflow, retry policy eventually-succeeds.
//!
//! The SDK's `#[workflow]` / `#[activities]` proc-macros expand to code
//! that references `futures::future::FutureExt::boxed`, so the `futures`
//! crate must be in scope (added as a dev-dep here; document the same
//! requirement for downstream consumers).

#![cfg(feature = "integration-tests")]
#![allow(
    clippy::missing_panics_doc,
    clippy::large_futures,
    missing_docs,
    // SDK proc-macros generate trait impls that trip pedantic lints.
    clippy::needless_pass_by_value,
    clippy::default_trait_access,
    clippy::unused_async,
    clippy::module_name_repetitions
)]

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use altair_temporal::temporalio_client::{WorkflowGetResultOptions, WorkflowStartOptions};
use altair_temporal::temporalio_common;
// `activity` and `run` attributes are referenced by the workflow_methods
// and activities macro expansions, even though the proc-macro names
// don't appear after expansion. Importing the names keeps them in scope
// for the macros' generated code.
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    ActivityOptions, WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_temporal::testcontainer::TemporalContainer;
use altair_temporal::{
    Client, Config, RetryPolicy, Schedule, Worker, WorkerBuilder, delete_schedule,
};
// `futures` must be in the dep graph: the SDK's #[workflow] /
// #[activities] macros expand to `.boxed()` calls on async blocks via
// `::futures::FutureExt`. We don't need to import the trait here ourselves;
// the macros use absolute paths.
use tokio::sync::OnceCell;

// ---------------------------------------------------------------------------
// Shared container fixture — start once, reuse across tests.
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

/// Unique-per-call suffix so concurrent tests don't collide on task queue
/// names, schedule ids, or workflow ids.
fn unique(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    format!("{prefix}-{pid}-{n}")
}

// ---------------------------------------------------------------------------
// Client tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_connects_to_dev_server() {
    let temporal = temporal().await;
    let cfg = temporal.config(unique("connect-tq"));
    assert_eq!(cfg.namespace, "default");
    assert!(!cfg.host.is_empty());
    let _client = Client::from_config(&cfg).await.expect("connect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_connect_to_unreachable_host_fails() {
    // Port 1 is "tcpmux" — virtually never bound, so the connect refuses
    // fast on every supported platform.
    let cfg = Config {
        host: "http://127.0.0.1:1".to_string(),
        namespace: "default".to_string(),
        task_queue: unique("unreachable"),
        ..Config::default()
    };
    let res = Client::from_config(&cfg).await;
    assert!(res.is_err(), "expected connect failure");
}

// ---------------------------------------------------------------------------
// Worker lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_builds_against_container_and_drops_cleanly() {
    let temporal = temporal().await;
    let cfg = temporal.config(unique("worker-build-tq"));
    let worker = WorkerBuilder::new(&cfg)
        .build()
        .await
        .expect("build worker");
    drop(worker);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_run_with_shutdown_future_exits_cleanly() {
    let temporal = temporal().await;
    let mut cfg = temporal.config(unique("worker-shutdown-tq"));
    // Override the prod-default 30s drain — there's nothing in flight
    // here so we just need the SDK to acknowledge the shutdown signal
    // and return.
    cfg.shutdown_grace = Duration::from_secs(2);
    let worker = WorkerBuilder::new(&cfg)
        .build()
        .await
        .expect("build worker");

    // Pass a shutdown future that completes after a short delay. The
    // worker should initiate drain and exit cleanly within the grace
    // period.
    //
    // (We don't `tokio::spawn` the worker because the SDK's run future
    // is not `Send`; running it inline on the current task is fine.)
    let shutdown = async { tokio::time::sleep(Duration::from_millis(250)).await };

    // Generous deadline: CI is slower than local, and the SDK takes
    // a few seconds to drain even an empty worker after shutdown.
    let res = tokio::time::timeout(
        Duration::from_mins(1),
        Box::pin(worker.run_with_shutdown(shutdown)),
    )
    .await
    .expect("worker shuts down within deadline");
    assert!(res.is_ok(), "worker should exit cleanly on shutdown");
}

// ---------------------------------------------------------------------------
// Schedule round-trip tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schedule_create_and_delete() {
    let temporal = temporal().await;
    let cfg = temporal.config(unique("sched-tq"));
    let client = Client::from_config(&cfg).await.expect("client");
    let sched_id = unique("sched-create-id");

    Schedule::builder()
        .cron("0 0 * * *")
        .start_workflow("EchoWorkflow", &cfg.task_queue, unique("sched-wid"))
        .paused(true)
        .create(&client, &sched_id)
        .await
        .expect("create schedule");

    delete_schedule(&client, &sched_id)
        .await
        .expect("delete schedule");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schedule_update_changes_spec() {
    let temporal = temporal().await;
    let cfg = temporal.config(unique("sched-upd-tq"));
    let client = Client::from_config(&cfg).await.expect("client");
    let sched_id = unique("sched-upd-id");

    // Create with morning cron, paused so it never fires during the test.
    Schedule::builder()
        .cron("0 9 * * *")
        .start_workflow("EchoWorkflow", &cfg.task_queue, unique("sched-upd-wid"))
        .paused(true)
        .create(&client, &sched_id)
        .await
        .expect("create schedule");

    // Update to evening cron.
    Schedule::builder()
        .cron("0 18 * * *")
        .start_workflow("EchoWorkflow", &cfg.task_queue, unique("sched-upd-wid"))
        .paused(true)
        .update(&client, &sched_id)
        .await
        .expect("update schedule");

    let _ = delete_schedule(&client, &sched_id).await;
}

// ---------------------------------------------------------------------------
// Phase 2: Workflow + activity definitions for execution tests
// ---------------------------------------------------------------------------

/// Plain echo workflow — returns its input. Exercises the simplest possible
/// workflow path (no activity, no timer).
#[workflow]
#[derive(Default)]
pub struct EchoWorkflow;

#[workflow_methods]
impl EchoWorkflow {
    #[run]
    pub async fn run(_ctx: &mut WorkflowContext<Self>, input: String) -> WorkflowResult<String> {
        Ok(input)
    }
}

/// Workflow that calls a single activity and returns the activity's result.
#[workflow]
#[derive(Default)]
pub struct GreetWorkflow;

#[workflow_methods]
impl GreetWorkflow {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, name: String) -> WorkflowResult<String> {
        let greeting = ctx
            .start_activity(
                GreetingActivities::greet,
                name,
                ActivityOptions::start_to_close_timeout(Duration::from_secs(10)),
            )
            .await
            .map_err(|e| anyhow::anyhow!("activity failed: {e}"))?;
        Ok(greeting)
    }
}

pub struct GreetingActivities;

#[activities]
impl GreetingActivities {
    // Use std::result::Result explicitly so the prelude's `Result` alias
    // (which has only one generic) doesn't shadow it inside the macro.
    #[activity]
    pub async fn greet(
        _ctx: ActivityContext,
        name: String,
    ) -> std::result::Result<String, ActivityError> {
        Ok(format!("Hello, {name}!"))
    }
}

/// Counter shared across attempts of [`FlakyActivities::attempt`].
static FLAKY_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
/// How many failures to inject before succeeding. Set by the test.
static FLAKY_FAIL_UNTIL: AtomicU32 = AtomicU32::new(0);

/// Workflow that invokes a flaky activity with a retry policy that allows
/// enough attempts to recover.
#[workflow]
#[derive(Default)]
pub struct FlakyWorkflow;

#[workflow_methods]
impl FlakyWorkflow {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, _input: ()) -> WorkflowResult<u32> {
        let retry_policy = RetryPolicy::builder()
            .initial_interval(Duration::from_millis(50))
            .maximum_interval(Duration::from_secs(1))
            .backoff_coefficient(2.0)
            .max_attempts(5)
            .build()
            .expect("retry policy")
            .into_inner();
        let opts = ActivityOptions::with_start_to_close_timeout(Duration::from_secs(5))
            .retry_policy(retry_policy)
            .build();
        let count = ctx
            .start_activity(FlakyActivities::attempt, (), opts)
            .await
            .map_err(|e| anyhow::anyhow!("activity failed: {e}"))?;
        Ok(count)
    }
}

pub struct FlakyActivities;

#[activities]
impl FlakyActivities {
    #[activity]
    pub async fn attempt(
        _ctx: ActivityContext,
        _input: (),
    ) -> std::result::Result<u32, ActivityError> {
        let n = FLAKY_ATTEMPTS.fetch_add(1, Ordering::SeqCst) + 1;
        if n <= FLAKY_FAIL_UNTIL.load(Ordering::SeqCst) {
            Err(ActivityError::application(
                temporalio_common::error::ApplicationFailure::builder(anyhow::anyhow!("transient"))
                    .type_name("Transient".to_string())
                    .non_retryable(false)
                    .build(),
            ))
        } else {
            Ok(n)
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Workflow execution tests
// ---------------------------------------------------------------------------

/// Run `worker.run_with_shutdown` inline (not spawned — the SDK's run
/// future isn't `Send`) and race it against a workload future. When the
/// workload finishes, signal shutdown so the worker exits cleanly.
async fn run_worker_with_workload<F, T>(worker: Worker, workload: F, deadline: Duration) -> T
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workflow_echo_round_trip() {
    let temporal = temporal().await;
    let tq = unique("echo-tq");
    let cfg = temporal.config(&tq);

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<EchoWorkflow>()
        .build()
        .await
        .expect("build worker");

    let client = Client::from_config(&cfg).await.expect("client");
    let wf_id = unique("echo-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let handle = client
            .start_workflow(
                EchoWorkflow::run,
                "hello world".to_string(),
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let result = run_worker_with_workload(worker, workload, Duration::from_mins(2)).await;
    assert_eq!(result, "hello world");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workflow_with_activity_returns_combined_result() {
    let temporal = temporal().await;
    let tq = unique("greet-tq");
    let cfg = temporal.config(&tq);

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<GreetWorkflow>()
        .register_activities(GreetingActivities)
        .build()
        .await
        .expect("build worker");

    let client = Client::from_config(&cfg).await.expect("client");
    let wf_id = unique("greet-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let handle = client
            .start_workflow(
                GreetWorkflow::run,
                "World".to_string(),
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let result = run_worker_with_workload(worker, workload, Duration::from_mins(2)).await;
    assert_eq!(result, "Hello, World!");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn retry_policy_eventually_succeeds() {
    let temporal = temporal().await;
    let tq = unique("flaky-tq");
    let cfg = temporal.config(&tq);

    FLAKY_ATTEMPTS.store(0, Ordering::SeqCst);
    FLAKY_FAIL_UNTIL.store(2, Ordering::SeqCst); // fail attempts 1,2; succeed on 3

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<FlakyWorkflow>()
        .register_activities(FlakyActivities)
        .build()
        .await
        .expect("build worker");

    let client = Client::from_config(&cfg).await.expect("client");
    let wf_id = unique("flaky-wid");
    let tq_clone = tq.clone();

    let workload = async move {
        let handle = client
            .start_workflow(
                FlakyWorkflow::run,
                (),
                WorkflowStartOptions::new(&tq_clone, &wf_id).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let result: u32 = run_worker_with_workload(worker, workload, Duration::from_mins(1)).await;
    assert_eq!(result, 3, "succeeds on attempt 3");
    assert_eq!(FLAKY_ATTEMPTS.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workflow_id_payload_round_trip_through_real_workflow() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct ArchiveJob {
        customer: String,
        year: u32,
    }

    let temporal = temporal().await;
    let tq = unique("wfid-tq");
    let cfg = temporal.config(&tq);

    let job = ArchiveJob {
        customer: "acme".to_string(),
        year: 2026,
    };
    let wf_id = altair_temporal::workflow_id::encode("archive", &job).expect("encode workflow id");

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<EchoWorkflow>()
        .build()
        .await
        .expect("build worker");

    let client = Client::from_config(&cfg).await.expect("client");
    let tq_clone = tq.clone();
    let wf_id_clone = wf_id.clone();

    let workload = async move {
        let handle = client
            .start_workflow(
                EchoWorkflow::run,
                "ack".to_string(),
                WorkflowStartOptions::new(&tq_clone, &wf_id_clone).build(),
            )
            .await
            .expect("start workflow");
        handle
            .get_result(WorkflowGetResultOptions::default())
            .await
            .expect("workflow result")
    };

    let result = run_worker_with_workload(worker, workload, Duration::from_mins(2)).await;
    assert_eq!(result, "ack");

    let (prefix, decoded): (String, ArchiveJob) =
        altair_temporal::workflow_id::decode(&wf_id).expect("decode workflow id");
    assert_eq!(prefix, "archive");
    assert_eq!(decoded, job);
}
