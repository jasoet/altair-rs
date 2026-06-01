//! `altair-wf` — `datasync::chunk` partitioned sync with continue-as-new.
//!
//! Walks 10 integer-keyed partitions, processing 3 per workflow execution
//! before issuing continue-as-new. A shared in-memory `ProgressTracker`
//! records the last completed partition end, so each fresh execution
//! skips the prefix that's already done. After 4 executions every
//! partition is processed exactly once.
//!
//! Prerequisite: `temporal server start-dev` running on `localhost:7233`.
//!
//! Run:
//! ```bash
//! cargo run -p altair-wf --features datasync --example datasync_chunked
//! ```

#![allow(missing_docs, clippy::unused_async)]

use std::sync::{Arc, Mutex};

use altair_temporal::WorkerBuilder;
use altair_temporal::temporalio_client::{Client, WorkflowGetResultOptions, WorkflowStartOptions};
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity, run, workflow, workflow_methods};
use altair_temporal::temporalio_sdk::{
    ContinueAsNewOptions, WorkflowContext, WorkflowResult,
    activities::{ActivityContext, ActivityError},
};
use altair_wf::TaskInput;
use altair_wf::datasync::chunk::{
    ChunkedSyncConfig, ChunkedSyncSummary, Cursor, Partition, PartitionResult, chunked_sync_run,
};

/// Shared state holding the partition list, the cursor, and recorded
/// fetch / advance calls so we can prove the cross-execution behaviour.
#[derive(Default)]
pub struct DemoState {
    pub partitions: Vec<Partition<i64>>,
    pub cursor: Mutex<Option<i64>>,
    pub fetches: Mutex<Vec<i64>>,
    pub advances: Mutex<Vec<i64>>,
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
        self.state.fetches.lock().unwrap().push(p.start);
        println!(
            "  run_partition activity: [{}, {}) on pid={}",
            p.start,
            p.end,
            std::process::id(),
        );
        // Simulate fetching 5 records; the sink "inserts" all of them.
        Ok(PartitionResult {
            start: p.start,
            end: p.end,
            fetched: 5,
            inserted: 5,
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
        self.state.advances.lock().unwrap().push(end);
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
pub struct DemoChunkedWf;

#[workflow_methods]
impl DemoChunkedWf {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: DemoInput,
    ) -> WorkflowResult<ChunkedSyncSummary<i64>> {
        let opts = altair_wf::default_activity_options();
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

        // The load-bearing part: when the helper truncated and set
        // `deferred = true`, hand the rest off to a fresh execution
        // with the same input. The cursor advanced during this run
        // lets the next execution skip the prefix.
        if result.deferred {
            ctx_ref.continue_as_new(&input, ContinueAsNewOptions::default())?;
            unreachable!("continue_as_new always returns Err");
        }
        Ok(result)
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    let cfg = altair_temporal::Config {
        task_queue: "altair-wf-datasync-chunked".to_string(),
        ..Default::default()
    };

    // 10 partitions: [0,10), [10,20), ... [90,100)
    let state = Arc::new(DemoState {
        partitions: (0..10)
            .map(|i| Partition::new(i * 10, (i + 1) * 10))
            .collect(),
        ..DemoState::default()
    });

    let worker = WorkerBuilder::new(&cfg)
        .register_workflow::<DemoChunkedWf>()
        .register_activities(DemoActivities {
            state: state.clone(),
        })
        .build()
        .await?;

    let client: Client = altair_temporal::Client::from_config(&cfg).await?;
    let input = DemoInput {
        job: "demo-chunked".into(),
        max_per_exec: 3,
    };
    let wf_id = format!("datasync-chunked-{}", std::process::id());
    let task_queue = cfg.task_queue.clone();
    let state_print = state.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let worker_fut = Box::pin(worker.run_with_shutdown(async move {
        let _ = shutdown_rx.await;
    }));
    let workload_fut = Box::pin(async move {
        let handle = client
            .start_workflow(
                DemoChunkedWf::run,
                input,
                WorkflowStartOptions::new(&task_queue, &wf_id).build(),
            )
            .await?;
        // get_result transparently follows continue_as_new chains.
        let out: ChunkedSyncSummary<i64> = handle
            .get_result(WorkflowGetResultOptions::default())
            .await?;
        let _ = shutdown_tx.send(());
        anyhow::Ok((wf_id, out))
    });

    let (worker_outcome, workload_outcome) = futures::future::join(worker_fut, workload_fut).await;
    worker_outcome?;
    let (wf_id, out) = workload_outcome?;

    println!();
    println!("workflow {wf_id} finished:");
    println!(
        "  final execution: total_partitions={}, deferred={}",
        out.total_partitions, out.deferred,
    );
    println!(
        "  fetches across all executions: {:?}",
        state_print.fetches.lock().unwrap(),
    );
    println!(
        "  advances across all executions: {:?}",
        state_print.advances.lock().unwrap(),
    );
    Ok(())
}
