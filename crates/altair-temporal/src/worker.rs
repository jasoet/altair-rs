//! Worker builder + lifecycle.

// `tracing::instrument` attaches a `Drop`-guarded span to the function
// body; under Rust 2024's tail-expr-drop-order rule this changes the
// drop order of locals borrowed by the tail expression. The worker
// futures we hand out don't carry side effects in `Drop` beyond the
// SDK's own cleanup, so the change is observationally neutral here.
#![allow(tail_expr_drop_order, clippy::single_match_else)]

use std::sync::Arc;

use temporalio_sdk_core::{CoreRuntime, FixedSizeSlotSupplier, RuntimeOptions, TunerBuilder};

use crate::client::Client;
use crate::config::Config;
use crate::error::{Error, Result};

type Registration = Box<
    dyn FnOnce(
            &mut temporalio_sdk::WorkerOptions,
        ) -> std::result::Result<(), temporalio_sdk::WorkflowRegistrationError>
        + Send,
>;

/// Builder for [`Worker`].
///
/// # Examples
///
/// ```no_run
/// # use altair_temporal::{Config, WorkerBuilder};
/// # async fn example() -> altair_temporal::Result<()> {
/// let cfg = Config { task_queue: "my-queue".into(), ..Default::default() };
/// let worker = WorkerBuilder::new(&cfg).build().await?;
/// worker.run().await?;
/// # Ok(())
/// # }
/// ```
pub struct WorkerBuilder {
    config: Config,
    registrations: Vec<Registration>,
}

impl WorkerBuilder {
    /// Start configuring a worker from a [`Config`].
    #[must_use]
    pub fn new(cfg: &Config) -> Self {
        Self {
            config: cfg.clone(),
            registrations: Vec::new(),
        }
    }

    /// Override identity. Use when you need a deterministic worker
    /// name for tests; production should leave this unset so each
    /// replica reports the SDK's `<pid>@<hostname>` default.
    #[must_use]
    pub fn identity(mut self, id: impl Into<String>) -> Self {
        self.config.identity = Some(id.into());
        self
    }

    /// Override max **concurrent** activities (slot capacity, not a
    /// rate limit).
    #[must_use]
    pub fn max_concurrent_activities(mut self, n: u32) -> Self {
        self.config.max_concurrent_activities = n;
        self
    }

    /// Override max **concurrent** workflow tasks (slot capacity, not
    /// the sticky-cache size — see
    /// [`max_cached_workflows`](Self::max_cached_workflows)).
    #[must_use]
    pub fn max_concurrent_workflows(mut self, n: u32) -> Self {
        self.config.max_concurrent_workflows = n;
        self
    }

    /// Override the sticky-cache LRU size.
    #[must_use]
    pub fn max_cached_workflows(mut self, n: usize) -> Self {
        self.config.max_cached_workflows = n;
        self
    }

    /// Register a workflow type for this worker.
    ///
    /// The trait bound matches the SDK's macro-generated `WorkflowImplementation`.
    #[must_use]
    pub fn register_workflow<W>(mut self) -> Self
    where
        W: temporalio_sdk::workflows::WorkflowImplementation + 'static,
        <W::Run as temporalio_common::WorkflowDefinition>::Input: Send,
    {
        self.registrations
            .push(Box::new(|opts| opts.register_workflow::<W>().map(|_| ())));
        self
    }

    /// Register an activity implementation instance.
    #[must_use]
    pub fn register_activities<A>(mut self, instance: A) -> Self
    where
        A: temporalio_sdk::activities::ActivityImplementer + Send + Sync + 'static,
    {
        self.registrations.push(Box::new(move |opts| {
            opts.register_activities(instance);
            Ok(())
        }));
        self
    }

    /// Build the worker: connect to Temporal, create the [`CoreRuntime`], and
    /// apply all queued workflow/activity registrations.
    ///
    /// # Errors
    ///
    /// Propagates [`Error::Connect`], [`Error::Configuration`], and
    /// [`Error::Worker`] from the underlying SDK.
    #[tracing::instrument(
        skip(self),
        fields(
            host = %self.config.host,
            namespace = %self.config.namespace,
            task_queue = %self.config.task_queue,
        ),
    )]
    pub async fn build(self) -> Result<Worker> {
        let client = Client::from_config(&self.config).await?;

        let runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::default())
            .map_err(|e| Error::worker(format!("runtime init: {e:#}")))?;

        // Build a Tuner with FixedSizeSlotSupplier for both workflow
        // and activity slots — these are the actual concurrency caps.
        // The SDK's per-second rate limit (`max_worker_activities_per_second`)
        // is intentionally NOT used: setting it silently throttles
        // workers to N exec/sec regardless of available parallelism.
        let workflow_slots =
            usize::try_from(self.config.max_concurrent_workflows).unwrap_or(usize::MAX);
        let activity_slots =
            usize::try_from(self.config.max_concurrent_activities).unwrap_or(usize::MAX);
        let mut tuner_builder = TunerBuilder::default();
        tuner_builder.workflow_slot_supplier(Arc::new(FixedSizeSlotSupplier::new(workflow_slots)));
        tuner_builder.activity_slot_supplier(Arc::new(FixedSizeSlotSupplier::new(activity_slots)));
        let tuner = Arc::new(tuner_builder.build());

        // Only set identity when the operator opted in — otherwise the
        // SDK picks `<pid>@<hostname>`, which is what we want in prod
        // so each replica is distinguishable in the Temporal UI.
        let mut worker_opts = temporalio_sdk::WorkerOptions::new(self.config.task_queue.clone())
            .max_cached_workflows(self.config.max_cached_workflows)
            .tuner(tuner)
            .maybe_graceful_shutdown_period(Some(self.config.shutdown_grace))
            .maybe_client_identity_override(self.config.identity.clone())
            .build();

        for reg in self.registrations {
            reg(&mut worker_opts)
                .map_err(|e| Error::worker(format!("workflow registration: {e}")))?;
        }

        let sdk_worker = temporalio_sdk::Worker::new(&runtime, client, worker_opts)
            .map_err(|e| Error::worker(format!("worker init: {e}")))?;

        let drain_deadline = self.config.shutdown_grace + std::time::Duration::from_secs(30);

        Ok(Worker {
            inner: sdk_worker,
            _runtime: runtime,
            drain_deadline,
        })
    }
}

/// A built Temporal worker ready to poll task queues.
pub struct Worker {
    inner: temporalio_sdk::Worker,
    /// Keeps the `CoreRuntime` alive for the worker's lifetime.
    _runtime: CoreRuntime,
    /// Outer cap on how long the drain may take after shutdown is
    /// initiated — set to `shutdown_grace + 30s` so we don't hang
    /// indefinitely if the SDK's run loop doesn't acknowledge
    /// `initiate_shutdown` for any reason.
    drain_deadline: std::time::Duration,
}

impl Worker {
    /// Run until SIGINT (`Ctrl-C`) or SIGTERM, then drain in-flight
    /// activities for up to [`Config::shutdown_grace`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Worker`] if the SDK worker exits with an error.
    pub async fn run(self) -> Result<()> {
        Box::pin(self.run_with_shutdown(shutdown_signal())).await
    }

    /// Run until `shutdown` resolves, then **gracefully drain**
    /// in-flight activities for up to [`Config::shutdown_grace`]
    /// before returning.
    ///
    /// Implementation: when the shutdown future fires, the worker's
    /// SDK-side `initiate_shutdown` token is set and the same worker
    /// future is then polled to completion. The SDK's
    /// `graceful_shutdown_period` controls the drain window — we
    /// configure it from [`Config::shutdown_grace`] in
    /// [`WorkerBuilder::build`].
    ///
    /// As a safety net, the drain is capped at
    /// `shutdown_grace + 30s`. If the SDK's run loop doesn't return
    /// by then, the worker future is dropped and a warning is logged.
    /// In a healthy worker this safety net never fires.
    ///
    /// Earlier versions of this method used `tokio::select!` to drop
    /// the worker future when shutdown fired — that cancelled
    /// in-flight activities mid-poll, leaving Temporal to mark them
    /// as start-to-close timeouts on the next retry. The drain pattern
    /// here avoids that.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Worker`] if the SDK worker exits with an error.
    #[tracing::instrument(skip(self, shutdown))]
    pub async fn run_with_shutdown<F>(mut self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let shutdown_handle = self.inner.shutdown_handle();
        let drain_deadline = self.drain_deadline;
        let mut run_fut = Box::pin(self.inner.run());
        tokio::select! {
            biased;
            res = &mut run_fut => return res.map_err(|e| Error::worker(format!("{e:#}"))),
            () = shutdown => {
                tracing::info!("shutdown requested; initiating graceful drain");
                shutdown_handle();
            }
        }
        // Continue polling the SAME worker future so the SDK's drain
        // window applies. Bound the wait with `drain_deadline` so we
        // don't hang if the SDK's run loop fails to acknowledge the
        // shutdown for any reason — the drop here is the SAME shape
        // the previous (drain-less) implementation had, so a stuck
        // worker is no worse than before.
        match tokio::time::timeout(drain_deadline, &mut run_fut).await {
            Ok(res) => {
                tracing::info!("worker drained");
                res.map_err(|e| Error::worker(format!("{e:#}")))
            }
            Err(_) => {
                tracing::warn!(
                    drain_deadline_secs = drain_deadline.as_secs(),
                    "drain deadline exceeded; dropping worker future",
                );
                Ok(())
            }
        }
    }
}

/// Resolves when SIGINT or SIGTERM is received.
///
/// If SIGTERM cannot be registered (rare — would require tokio's signal
/// machinery to be unavailable), the worker still shuts down on SIGINT.
async fn shutdown_signal() {
    use tokio::signal::ctrl_c;
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = ctrl_c() => {},
                    _ = term.recv() => {},
                }
            }
            Err(e) => {
                tracing::warn!(
                    "failed to install SIGTERM handler ({e}); falling back to SIGINT only"
                );
                let _ = ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c().await;
    }
}
