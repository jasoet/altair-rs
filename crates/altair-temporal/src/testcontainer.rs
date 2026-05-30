//! Reusable [`testcontainers`]-based fixture for Temporal integration tests.
//!
//! Enable the `testcontainers` feature in your `[dev-dependencies]`:
//!
//! ```toml
//! [dev-dependencies]
//! altair-temporal = { version = "0.1", features = ["testcontainers"] }
//! tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
//! ```
//!
//! Then start a container in a test:
//!
//! ```no_run
//! # #[cfg(feature = "testcontainers")] {
//! use altair_temporal::testcontainer::TemporalContainer;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let temporal = TemporalContainer::start().await?;
//! let cfg = temporal.config("my-task-queue");
//! let client = altair_temporal::Client::from_config(&cfg).await?;
//! # Ok(()) }
//! # }
//! ```
//!
//! The fixture pulls `temporalio/auto-setup` (all-in-one dev image: server +
//! UI + auto-namespace bootstrap) and waits until the frontend gRPC port is
//! ready. The default namespace is `default`. Drop the [`TemporalContainer`]
//! to stop the container.

use std::time::Duration;

use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};

use crate::config::Config;

/// Image tag used by [`TemporalContainer::start`]. Pinned to a known-good
/// release for reproducibility; override with [`TemporalContainer::builder`]
/// to upgrade. Refers to the Temporal CLI image, which bundles the dev
/// server with embedded `SQLite`.
pub const DEFAULT_IMAGE_TAG: &str = "latest";

/// Default Temporal namespace created by `server start-dev`.
pub const DEFAULT_NAMESPACE: &str = "default";

/// Temporal frontend gRPC port inside the container.
const FRONTEND_PORT: u16 = 7233;

/// A running Temporal dev server in a Docker container.
///
/// Holds the [`ContainerAsync`] handle; the container is stopped when the
/// `TemporalContainer` is dropped.
///
/// Start one per test (cheap container) or share across a suite via
/// `tokio::sync::OnceCell` (faster but tests must use unique task queues to
/// avoid cross-contamination).
pub struct TemporalContainer {
    container: ContainerAsync<GenericImage>,
    host: String,
    port: u16,
    namespace: String,
}

impl TemporalContainer {
    /// Start a container with default settings (image `temporalio/auto-setup`,
    /// tag [`DEFAULT_IMAGE_TAG`], namespace [`DEFAULT_NAMESPACE`]).
    ///
    /// # Errors
    ///
    /// Returns an error if Docker is unavailable, the image cannot be pulled,
    /// or the container fails to report readiness within the startup window.
    pub async fn start() -> Result<Self, TestcontainerError> {
        Self::builder().start().await
    }

    /// Begin configuring a container.
    #[must_use]
    pub fn builder() -> TemporalContainerBuilder {
        TemporalContainerBuilder::default()
    }

    /// Docker host where the container is reachable (typically `localhost`
    /// or `127.0.0.1`).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Mapped port for the Temporal frontend gRPC service.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Frontend gRPC URL, e.g. `http://127.0.0.1:54321`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Namespace created by `auto-setup` (always [`DEFAULT_NAMESPACE`] unless
    /// overridden).
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Pre-populated [`Config`] for connecting [`crate::Client`] /
    /// [`crate::WorkerBuilder`] to this container.
    ///
    /// `task_queue` is the queue this worker/client targets — pick a unique
    /// string per test if you share a container across tests.
    #[must_use]
    pub fn config(&self, task_queue: impl Into<String>) -> Config {
        Config {
            host: self.url(),
            namespace: self.namespace.clone(),
            task_queue: task_queue.into(),
            ..Config::default()
        }
    }

    /// The underlying [`ContainerAsync`] for advanced operations (exec,
    /// log streaming, restart).
    #[must_use]
    pub fn container(&self) -> &ContainerAsync<GenericImage> {
        &self.container
    }
}

/// Builder for [`TemporalContainer`].
pub struct TemporalContainerBuilder {
    image: String,
    tag: String,
    namespace: String,
    startup_timeout: Duration,
}

impl Default for TemporalContainerBuilder {
    fn default() -> Self {
        Self {
            image: "temporalio/temporal".to_string(),
            tag: DEFAULT_IMAGE_TAG.to_string(),
            namespace: DEFAULT_NAMESPACE.to_string(),
            startup_timeout: Duration::from_mins(2),
        }
    }
}

impl TemporalContainerBuilder {
    /// Override the image name (default `temporalio/auto-setup`).
    #[must_use]
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }

    /// Override the image tag (default [`DEFAULT_IMAGE_TAG`]).
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Override the namespace name passed to `auto-setup` (default
    /// [`DEFAULT_NAMESPACE`]).
    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Override the maximum time to wait for the container to report
    /// readiness (default 120s).
    #[must_use]
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = d;
        self
    }

    /// Start the container.
    ///
    /// # Errors
    ///
    /// Returns an error if Docker is unavailable, the image cannot be pulled,
    /// or the container fails to report readiness before
    /// `startup_timeout` elapses.
    pub async fn start(self) -> Result<TemporalContainer, TestcontainerError> {
        // `temporal server start-dev` prints "Temporal Server:" once the
        // frontend gRPC service is bound and accepting connections.
        let ready_log = WaitFor::message_on_stdout("Temporal Server:");

        let image = GenericImage::new(&self.image, &self.tag)
            .with_exposed_port(ContainerPort::Tcp(FRONTEND_PORT))
            .with_wait_for(ready_log)
            .with_cmd([
                "server",
                "start-dev",
                "--ip",
                "0.0.0.0",
                "--namespace",
                &self.namespace,
            ])
            .with_startup_timeout(self.startup_timeout);

        let container = image
            .start()
            .await
            .map_err(|e| TestcontainerError::Start(format!("{e}")))?;

        let host = container
            .get_host()
            .await
            .map_err(|e| TestcontainerError::Inspect(format!("{e}")))?
            .to_string();

        let port = container
            .get_host_port_ipv4(FRONTEND_PORT)
            .await
            .map_err(|e| TestcontainerError::Inspect(format!("{e}")))?;

        Ok(TemporalContainer {
            container,
            host,
            port,
            namespace: self.namespace,
        })
    }
}

/// Errors raised by [`TemporalContainer`].
#[derive(Debug, thiserror::Error)]
pub enum TestcontainerError {
    /// The container failed to start (Docker unreachable, image missing,
    /// timeout waiting for the readiness log line, etc.).
    #[error("failed to start Temporal container: {0}")]
    Start(String),
    /// Inspecting a started container failed.
    #[error("failed to inspect Temporal container: {0}")]
    Inspect(String),
}
