//! Workflow-side helper that drives a chunked sync: list -> cursor-filter
//! -> truncate-to-max -> iterate(fetch/map/write + advance) -> return
//! with `deferred = true` when more partitions remain.
//!
//! The helper is **SDK-agnostic** — it takes async closures for each
//! step so the workflow body wires its own activity calls. The caller
//! is responsible for issuing
//! [`WorkflowContext::continue_as_new`](altair_temporal::temporalio_sdk::WorkflowContext::continue_as_new)
//! when the returned summary's `deferred` flag is set.
//!
//! # Determinism inside a Temporal workflow
//!
//! Every closure handed to [`chunked_sync_run`] runs inside the
//! workflow body and therefore must respect Temporal's deterministic
//! replay rules. In practice:
//!
//! - `list_partitions`, `run_partition`, the cursor `read` / `advance`
//!   closures should wrap activity dispatches
//!   (`ctx.start_activity(...).await`) — never call external services
//!   directly from the closure body.
//! - The `sleeper` closure **must** use the workflow-side timer
//!   (`ctx.timer(d)` or equivalent), never `tokio::time::sleep` or
//!   `std::thread::sleep` — those break replay determinism.

use std::future::Future;
use std::time::Duration;

use crate::datasync::chunk::partition::{Partition, PartitionResult, SyncResult};
use crate::error::Result;

/// Configuration for [`chunked_sync_run`].
///
/// Fields are private — use [`ChunkedSyncConfig::new`] plus the
/// chainable setters (`partition_sleep`, `max_partitions_per_execution`)
/// to construct. Keeping the fields private removes the field/setter
/// name overlap that would otherwise let two `cfg.partition_sleep` call
/// sites mean different things.
#[derive(Debug, Clone)]
pub struct ChunkedSyncConfig {
    /// Echoed into [`SyncResult::job_name`].
    pub(crate) job_name: String,
    /// Sleep duration inserted between partition activity calls. Zero
    /// disables the inter-partition delay.
    ///
    /// The sleep itself is performed by the `sleeper` closure passed
    /// to [`chunked_sync_run`]. Inside a Temporal workflow body, that
    /// closure must use the workflow-side timer (`ctx.timer(d)`) so
    /// replay stays deterministic.
    pub(crate) partition_sleep: Duration,
    /// Cap on partitions handled by one execution. When `> 0` and the
    /// partition list (after cursor filtering) exceeds this number, the
    /// helper truncates to this many and sets `deferred = true` on the
    /// returned summary — the caller must then issue continue-as-new
    /// with the same workflow input so the rest gets processed.
    ///
    /// **Must be paired with a cursor.** Without one, the next execution
    /// would re-process the same prefix forever.
    pub(crate) max_partitions_per_execution: usize,
}

impl ChunkedSyncConfig {
    /// Build a config with the given job name and no extras.
    #[must_use]
    pub fn new(job_name: impl Into<String>) -> Self {
        Self {
            job_name: job_name.into(),
            partition_sleep: Duration::ZERO,
            max_partitions_per_execution: 0,
        }
    }

    /// Sleep between partitions (caller-provided sleeper).
    #[must_use]
    pub fn partition_sleep(mut self, sleep: Duration) -> Self {
        self.partition_sleep = sleep;
        self
    }

    /// Cap partitions handled per execution.
    #[must_use]
    pub fn max_partitions_per_execution(mut self, n: usize) -> Self {
        self.max_partitions_per_execution = n;
        self
    }
}

/// Cursor-handling shape. Pair `read` + `advance` to enable resumable
/// chunked sync; pass [`Cursor::None`] when the partitioner is bounded
/// and there is no need to skip processed work.
///
/// **Closure shape.** The `ReadFn` / `AdvFn` generics are not bounded
/// at the type level — they get their full bounds inside the
/// [`chunked_sync_run`] helper. The expected shape is:
///
/// ```text
/// ReadFn: FnOnce() -> Future<Output = Result<Option<K>>>
/// AdvFn:  FnMut(K) -> Future<Output = Result<()>>
/// ```
///
/// **Inclusion semantics.** The cursor is interpreted as "every
/// partition whose `start >= cursor` is unprocessed". A partition with
/// `start` exactly equal to the cursor is **kept** (processed); the
/// `advance` closure should therefore record `partition.end`, not
/// `partition.start`. This mirrors the Go original at
/// `go-wf/datasync/chunk/sync.go`.
///
/// **Idempotency.** `advance` may be invoked more than once for the
/// same `completed` value if the workflow retries after a transient
/// activity failure — implementations should be idempotent (an
/// "INSERT … ON CONFLICT DO NOTHING" pattern, an upsert against a
/// monotone column, or equivalent).
pub enum Cursor<ReadFn, AdvFn> {
    /// No cursor — process every partition the partitioner returns.
    ///
    /// Incompatible with `ChunkedSyncConfig::max_partitions_per_execution > 0`;
    /// the helper returns [`Error::InvalidInput`](crate::error::Error::InvalidInput)
    /// in that combination.
    None,
    /// `read` returns the last completed end-key (or `None` if no cursor
    /// has been recorded). `advance` is invoked after each successful
    /// partition with that partition's `end` key.
    Some {
        /// Read the current cursor value. Returning `Ok(None)` reports
        /// "no cursor yet" — every partition will be processed.
        read: ReadFn,
        /// Record progress past `completed` (always a partition `end`).
        /// Must be idempotent; see the type-level note above.
        advance: AdvFn,
    },
}

/// Run a chunked sync.
///
/// The function does **not** call any Temporal SDK directly — it
/// composes the caller-provided async closures into the orchestration
/// shape (list -> filter -> truncate -> iterate -> advance -> sleep).
/// Inside a Temporal workflow body, each closure typically wraps a
/// `start_activity(...).await` call; outside a workflow (in tests or
/// scripts), the closures can call their concrete services directly.
///
/// # Why 11 generics?
///
/// Five distinct async closures (list, run, read, advance, sleep) each
/// expand into a `Fn(_) -> Fut` pair (10) plus the partition key `K`,
/// for 11 type parameters. Rust does not currently let us collapse
/// these into a single trait without imposing structural change on the
/// call sites. The verbose signature is the price of keeping the helper
/// SDK-agnostic. If the count becomes a problem, split into
/// `chunked_sync_run_with_cursor` and `chunked_sync_run_no_cursor`
/// helpers — see the deferred-fix log.
///
/// # Errors
///
/// - [`Error::InvalidInput`](crate::error::Error::InvalidInput) when
///   `config.max_partitions_per_execution > 0` is set without a cursor.
/// - Anything `list_partitions`, `run_partition`, the cursor read, the
///   cursor advance, or the sleeper returns.
///
/// # Partial progress on activity error
///
/// When `run_partition` fails on the *N*-th partition, every partition
/// before it has already been processed *and* `advance` has been
/// called for it (the helper does **not** roll back the cursor).
/// A subsequent execution that retries the workflow input will skip
/// past those completed partitions via cursor filtering — so the
/// tracker is the durable boundary, and `advance` implementations
/// must be idempotent (see [`Cursor`] for the type-level note).
#[allow(clippy::too_many_arguments)]
pub async fn chunked_sync_run<
    K,
    ListFut,
    RunFut,
    ReadFut,
    AdvFut,
    SleepFut,
    ListFn,
    RunFn,
    ReadFn,
    AdvFn,
    SleepFn,
>(
    config: ChunkedSyncConfig,
    list_partitions: ListFn,
    mut run_partition: RunFn,
    cursor: Cursor<ReadFn, AdvFn>,
    mut sleeper: SleepFn,
) -> Result<SyncResult<K>>
where
    K: Ord + Clone + Send + Sync + 'static,
    ListFn: FnOnce() -> ListFut,
    ListFut: Future<Output = Result<Vec<Partition<K>>>>,
    RunFn: FnMut(Partition<K>) -> RunFut,
    RunFut: Future<Output = Result<PartitionResult<K>>>,
    ReadFn: FnOnce() -> ReadFut,
    ReadFut: Future<Output = Result<Option<K>>>,
    AdvFn: FnMut(K) -> AdvFut,
    AdvFut: Future<Output = Result<()>>,
    SleepFn: FnMut(Duration) -> SleepFut,
    SleepFut: Future<Output = ()>,
{
    if config.max_partitions_per_execution > 0 && matches!(cursor, Cursor::None) {
        return Err(crate::error::Error::InvalidInput(
            "max_partitions_per_execution requires a cursor — without one, the next execution would re-process the same prefix forever".into(),
        ));
    }

    let mut summary = SyncResult::<K> {
        job_name: config.job_name.clone(),
        ..SyncResult::default()
    };

    let mut parts = list_partitions().await?;
    if parts.is_empty() {
        return Ok(summary);
    }

    // Cursor filter.
    let mut advance = match cursor {
        Cursor::Some { read, advance } => {
            let cur = read().await?;
            if let Some(c) = cur {
                parts.retain(|p| p.start >= c);
            }
            if parts.is_empty() {
                return Ok(summary);
            }
            Some(advance)
        }
        Cursor::None => None,
    };

    // Truncate.
    let deferred = config.max_partitions_per_execution > 0
        && parts.len() > config.max_partitions_per_execution;
    if deferred {
        parts.truncate(config.max_partitions_per_execution);
    }

    let n = parts.len();
    for (i, p) in parts.into_iter().enumerate() {
        let end_for_advance = p.end.clone();
        let pr = run_partition(p).await?;
        summary.total_partitions += 1;
        summary.total_fetched += pr.fetched;
        summary.total_inserted += pr.inserted;
        summary.total_updated += pr.updated;
        summary.total_skipped += pr.skipped;
        summary.partitions.push(pr);

        if let Some(adv) = advance.as_mut() {
            adv(end_for_advance).await?;
        }

        if i < n - 1 && !config.partition_sleep.is_zero() {
            sleeper(config.partition_sleep).await;
        }
    }

    summary.deferred = deferred;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr<K: Clone>(start: K, end: K, fetched: usize, inserted: usize) -> PartitionResult<K> {
        PartitionResult {
            start,
            end,
            fetched,
            inserted,
            updated: 0,
            skipped: 0,
        }
    }

    type NoCursorRead = fn() -> std::future::Ready<Result<Option<i64>>>;
    type NoCursorAdv = fn(i64) -> std::future::Ready<Result<()>>;

    #[tokio::test]
    async fn run_without_tracker_visits_every_partition() {
        let parts = vec![Partition::new(0_i64, 1), Partition::new(1, 2)];
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j"),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 10, 10)) },
            Cursor::<NoCursorRead, NoCursorAdv>::None,
            |_d| async {},
        )
        .await
        .unwrap();
        assert_eq!(result.total_partitions, 2);
        assert_eq!(result.total_fetched, 20);
        assert!(!result.deferred);
    }

    #[tokio::test]
    async fn empty_partition_list_short_circuits() {
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j"),
            || async move { Ok(Vec::<Partition<i64>>::new()) },
            |p: Partition<i64>| async move { Ok(pr(p.start, p.end, 0, 0)) },
            Cursor::<NoCursorRead, NoCursorAdv>::None,
            |_d| async {},
        )
        .await
        .unwrap();
        assert_eq!(result.total_partitions, 0);
        assert!(result.partitions.is_empty());
    }

    #[tokio::test]
    async fn cursor_filter_skips_already_processed_partitions() {
        let parts = vec![
            Partition::new(0_i64, 10),
            Partition::new(10, 20),
            Partition::new(20, 30),
        ];
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j"),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 1, 1)) },
            Cursor::Some {
                read: || async { Ok(Some(15_i64)) },
                advance: |_end: i64| async { Ok(()) },
            },
            |_d| async {},
        )
        .await
        .unwrap();
        // start>=15 keeps [20,30).
        assert_eq!(result.total_partitions, 1);
        assert_eq!(result.partitions[0].start, 20);
    }

    #[tokio::test]
    async fn max_partitions_truncates_and_sets_deferred() {
        let parts = (0_i64..10)
            .map(|i| Partition::new(i, i + 1))
            .collect::<Vec<_>>();
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j").max_partitions_per_execution(3),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 1, 1)) },
            Cursor::Some {
                read: || async { Ok(None) },
                advance: |_end: i64| async { Ok(()) },
            },
            |_d| async {},
        )
        .await
        .unwrap();
        assert_eq!(result.total_partitions, 3);
        assert!(result.deferred);
    }

    #[tokio::test]
    async fn max_partitions_without_cursor_rejects() {
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j").max_partitions_per_execution(3),
            || async move { Ok(vec![Partition::new(0_i64, 1)]) },
            |p: Partition<i64>| async move { Ok(pr(p.start, p.end, 1, 1)) },
            Cursor::<NoCursorRead, NoCursorAdv>::None,
            |_d| async {},
        )
        .await;
        assert!(matches!(result, Err(crate::error::Error::InvalidInput(_))));
    }

    #[tokio::test]
    async fn cursor_advance_is_invoked_per_partition_with_partition_end() {
        let parts = vec![Partition::new(0_i64, 10), Partition::new(10, 20)];
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let cap = seen.clone();
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j"),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 1, 1)) },
            Cursor::Some {
                read: || async { Ok(None) },
                advance: move |end: i64| {
                    let cap = cap.clone();
                    async move {
                        cap.lock().unwrap().push(end);
                        Ok(())
                    }
                },
            },
            |_d| async {},
        )
        .await
        .unwrap();
        assert_eq!(result.total_partitions, 2);
        assert_eq!(*seen.lock().unwrap(), vec![10, 20]);
    }

    #[tokio::test]
    async fn cursor_at_partition_start_is_inclusive() {
        // Regression: cursor == partition.start retains that partition
        // (filter is `p.start >= c`). Pins the inclusive-on-start
        // semantic documented on `Cursor::Some::read`.
        let parts = vec![
            Partition::new(0_i64, 10),
            Partition::new(10, 20),
            Partition::new(20, 30),
        ];
        let result = chunked_sync_run(
            ChunkedSyncConfig::new("j"),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 1, 1)) },
            Cursor::Some {
                read: || async { Ok(Some(10_i64)) },
                advance: |_end: i64| async { Ok(()) },
            },
            |_d| async {},
        )
        .await
        .unwrap();
        // cursor=10 keeps starts 10 and 20 — i.e. [10,20) and [20,30).
        assert_eq!(result.total_partitions, 2);
        assert_eq!(result.partitions[0].start, 10);
        assert_eq!(result.partitions[1].start, 20);
    }

    #[tokio::test]
    async fn list_partitions_error_propagates_to_caller() {
        let result = chunked_sync_run::<i64, _, _, _, _, _, _, _, NoCursorRead, NoCursorAdv, _>(
            ChunkedSyncConfig::new("j"),
            || async move {
                Err::<Vec<Partition<i64>>, _>(crate::error::Error::InvalidInput("nope".into()))
            },
            |p| async move { Ok(pr(p.start, p.end, 0, 0)) },
            Cursor::None,
            |_d| async {},
        )
        .await;
        assert!(matches!(result, Err(crate::error::Error::InvalidInput(_))));
    }

    #[tokio::test]
    async fn partition_sleep_invoked_n_minus_1_times() {
        let parts = vec![
            Partition::new(0_i64, 1),
            Partition::new(1, 2),
            Partition::new(2, 3),
        ];
        let sleeps = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cap = sleeps.clone();
        chunked_sync_run(
            ChunkedSyncConfig::new("j").partition_sleep(Duration::from_millis(1)),
            || async move { Ok(parts) },
            |p| async move { Ok(pr(p.start, p.end, 0, 0)) },
            Cursor::<NoCursorRead, NoCursorAdv>::None,
            move |_d| {
                let cap = cap.clone();
                async move {
                    cap.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(sleeps.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
