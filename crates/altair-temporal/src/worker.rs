//! Worker builder + lifecycle.

use temporalio_sdk_core::{CoreRuntime, RuntimeOptions};

use crate::client::Client;
use crate::config::Config;
use crate::error::{Error, Result};

type Registration = Box<dyn FnOnce(&mut temporalio_sdk::WorkerOptions) + Send>;

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

    /// Override identity (defaults to the value in `Config`).
    #[must_use]
    pub fn identity(mut self, id: impl Into<String>) -> Self {
        self.config.identity = id.into();
        self
    }

    /// Override max concurrent activities.
    #[must_use]
    pub fn max_concurrent_activities(mut self, n: u32) -> Self {
        self.config.max_concurrent_activities = n;
        self
    }

    /// Override max concurrent workflows (cached workflow slots).
    #[must_use]
    pub fn max_concurrent_workflows(mut self, n: u32) -> Self {
        self.config.max_concurrent_workflows = n;
        self
    }

    /// Register a workflow type for this worker.
    ///
    /// The trait bound matches the SDK's macro-generated `WorkflowImplementer`.
    #[must_use]
    pub fn register_workflow<W>(mut self) -> Self
    where
        W: temporalio_sdk::workflows::WorkflowImplementer + 'static,
    {
        self.registrations.push(Box::new(|opts| {
            opts.register_workflow::<W>();
        }));
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
    pub async fn build(self) -> Result<Worker> {
        let client = Client::from_config(&self.config).await?;

        let runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::default())
            .map_err(|e| Error::worker(format!("runtime init: {e:#}")))?;

        let max_cached = usize::try_from(self.config.max_concurrent_workflows)
            .unwrap_or(usize::MAX);
        #[allow(clippy::cast_precision_loss)]
        let max_activities_per_sec = self.config.max_concurrent_activities as f64;

        let mut worker_opts = temporalio_sdk::WorkerOptions::new(self.config.task_queue.clone())
            .max_cached_workflows(max_cached)
            .maybe_max_worker_activities_per_second(Some(max_activities_per_sec))
            .maybe_client_identity_override(Some(self.config.identity.clone()))
            .build();

        for reg in self.registrations {
            reg(&mut worker_opts);
        }

        let sdk_worker = temporalio_sdk::Worker::new(&runtime, client, worker_opts)
            .map_err(|e| Error::worker(format!("worker init: {e}")))?;

        Ok(Worker {
            inner: sdk_worker,
            _runtime: runtime,
        })
    }
}

/// A built Temporal worker ready to poll task queues.
pub struct Worker {
    inner: temporalio_sdk::Worker,
    /// Keeps the `CoreRuntime` alive for the worker's lifetime.
    _runtime: CoreRuntime,
}

impl Worker {
    /// Run until SIGINT (`Ctrl-C`) or SIGTERM.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Worker`] if the SDK worker exits with an error.
    pub async fn run(self) -> Result<()> {
        self.run_with_shutdown(shutdown_signal()).await
    }

    /// Run until the given future resolves.
    ///
    /// `tokio::select!` cancels the losing branch; the SDK's
    /// `graceful_shutdown_period` setting handles in-flight activity cleanup.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Worker`] if the SDK worker exits with an error.
    pub async fn run_with_shutdown<F>(mut self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        tokio::select! {
            res = self.inner.run() => res.map_err(|e| Error::worker(format!("{e:#}"))),
            () = shutdown => Ok(()),
        }
    }
}

/// Resolves when SIGINT or SIGTERM is received.
async fn shutdown_signal() {
    use tokio::signal::ctrl_c;
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c() => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c().await;
    }
}
