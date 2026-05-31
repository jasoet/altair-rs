//! Reusable [`testcontainers`]-based fixture for `OpenTelemetry` Collector
//! integration tests.
//!
//! Enable the `testcontainers` feature in your `[dev-dependencies]`:
//!
//! ```toml
//! [dev-dependencies]
//! altair-otel = { version = "0.1", features = ["testcontainers"] }
//! tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
//! ```
//!
//! Then start a container in a test:
//!
//! ```no_run
//! # #[cfg(feature = "testcontainers")] {
//! use altair_otel::testcontainer::OtelCollectorContainer;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let collector = OtelCollectorContainer::start().await?;
//! let cfg = altair_otel::Config::builder()
//!     .service_name("my-service")
//!     .otlp_endpoint(collector.grpc_endpoint())
//!     .build();
//! cfg.init()?;
//! # Ok(()) }
//! # }
//! ```
//!
//! The fixture pulls `otel/opentelemetry-collector` and exposes the
//! default OTLP gRPC port (4317) and HTTP port (4318). Drop the
//! [`OtelCollectorContainer`] to stop the container.

use std::time::Duration;

use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};

/// Image tag used by [`OtelCollectorContainer::start`]. Override with
/// [`OtelCollectorContainer::builder`] to upgrade.
pub const DEFAULT_IMAGE_TAG: &str = "latest";

/// OTLP gRPC port inside the container.
const GRPC_PORT: u16 = 4317;

/// OTLP HTTP port inside the container.
const HTTP_PORT: u16 = 4318;

/// A running OpenTelemetry Collector in a Docker container, accepting
/// OTLP traces / metrics / logs on the standard gRPC + HTTP ports.
///
/// Holds the [`ContainerAsync`] handle; the container is stopped when
/// the `OtelCollectorContainer` is dropped.
pub struct OtelCollectorContainer {
    container: ContainerAsync<GenericImage>,
    host: String,
    grpc_port: u16,
    http_port: u16,
}

impl OtelCollectorContainer {
    /// Start a container with default settings (image
    /// `otel/opentelemetry-collector`, tag [`DEFAULT_IMAGE_TAG`]). The
    /// collector's default config exposes an OTLP receiver on gRPC :4317
    /// and HTTP :4318 — ready for exporters to push data.
    ///
    /// # Errors
    ///
    /// Returns an error if Docker is unavailable, the image cannot be
    /// pulled, or the collector fails to report readiness within the
    /// startup window.
    pub async fn start() -> Result<Self, TestcontainerError> {
        Self::builder().start().await
    }

    /// Begin configuring a container.
    #[must_use]
    pub fn builder() -> OtelCollectorContainerBuilder {
        OtelCollectorContainerBuilder::default()
    }

    /// Docker host where the container is reachable (typically
    /// `localhost` or `127.0.0.1`).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Mapped host port for the OTLP gRPC service.
    #[must_use]
    pub fn grpc_port(&self) -> u16 {
        self.grpc_port
    }

    /// Mapped host port for the OTLP HTTP service.
    #[must_use]
    pub fn http_port(&self) -> u16 {
        self.http_port
    }

    /// OTLP gRPC endpoint URL, e.g. `http://127.0.0.1:54321`. Pass this
    /// to `Config::builder().otlp_endpoint(...)` when the `otlp-grpc`
    /// feature is enabled.
    #[must_use]
    pub fn grpc_endpoint(&self) -> String {
        format!("http://{}:{}", self.host, self.grpc_port)
    }

    /// OTLP HTTP endpoint URL, e.g. `http://127.0.0.1:54322`. Pass this
    /// when the `otlp-http` feature is enabled.
    #[must_use]
    pub fn http_endpoint(&self) -> String {
        format!("http://{}:{}", self.host, self.http_port)
    }

    /// The underlying [`ContainerAsync`] for advanced operations (exec,
    /// log streaming, restart).
    #[must_use]
    pub fn container(&self) -> &ContainerAsync<GenericImage> {
        &self.container
    }
}

/// Builder for [`OtelCollectorContainer`].
pub struct OtelCollectorContainerBuilder {
    image: String,
    tag: String,
    startup_timeout: Duration,
}

impl Default for OtelCollectorContainerBuilder {
    fn default() -> Self {
        Self {
            image: "otel/opentelemetry-collector".to_string(),
            tag: DEFAULT_IMAGE_TAG.to_string(),
            startup_timeout: Duration::from_mins(1),
        }
    }
}

impl OtelCollectorContainerBuilder {
    /// Override the image name (default `otel/opentelemetry-collector`).
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

    /// Override the maximum time to wait for the container to report
    /// readiness (default 60s).
    #[must_use]
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = d;
        self
    }

    /// Start the container.
    ///
    /// # Errors
    ///
    /// Returns an error if Docker is unavailable, the image cannot be
    /// pulled, or the collector fails to report readiness within
    /// `startup_timeout`.
    pub async fn start(self) -> Result<OtelCollectorContainer, TestcontainerError> {
        // The collector prints this once all receivers/exporters are
        // started and accepting data.
        let ready_log =
            WaitFor::message_on_stderr("Everything is ready. Begin running and processing data.");

        let image = GenericImage::new(&self.image, &self.tag)
            .with_exposed_port(ContainerPort::Tcp(GRPC_PORT))
            .with_exposed_port(ContainerPort::Tcp(HTTP_PORT))
            .with_wait_for(ready_log)
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

        let grpc_port = container
            .get_host_port_ipv4(GRPC_PORT)
            .await
            .map_err(|e| TestcontainerError::Inspect(format!("{e}")))?;

        let http_port = container
            .get_host_port_ipv4(HTTP_PORT)
            .await
            .map_err(|e| TestcontainerError::Inspect(format!("{e}")))?;

        Ok(OtelCollectorContainer {
            container,
            host,
            grpc_port,
            http_port,
        })
    }
}

/// Errors raised by [`OtelCollectorContainer`].
#[derive(Debug, thiserror::Error)]
pub enum TestcontainerError {
    /// The container failed to start (Docker unreachable, image missing,
    /// timeout waiting for the readiness log line, etc.).
    #[error("failed to start OTel collector container: {0}")]
    Start(String),
    /// Inspecting a started container failed.
    #[error("failed to inspect OTel collector container: {0}")]
    Inspect(String),
}
