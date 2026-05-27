//! `OTel` setup configuration.

use crate::error::{Error, Result};

/// How span/log/metric data is exported.
#[derive(Debug, Clone, Default)]
pub enum Exporter {
    /// OTLP over gRPC (default).
    #[default]
    Otlp,
    /// Stdout exporter for local dev.
    Stdout,
    /// No exporter — spans/logs are dropped (useful for tests).
    None,
}

/// Log output format on stdout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable, multi-line pretty format.
    #[default]
    Pretty,
    /// JSON, one object per line.
    Json,
}

/// `OTel` setup config.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) service_name: String,
    pub(crate) service_version: Option<String>,
    pub(crate) otlp_endpoint: Option<String>,
    pub(crate) resource_attributes: Vec<(String, String)>,
    pub(crate) exporter: Exporter,
    pub(crate) log_format: LogFormat,
}

impl Config {
    /// Start building a config.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Build a config from `OTel`-spec environment variables.
    ///
    /// Reads:
    /// - `OTEL_SERVICE_NAME` (required)
    /// - `OTEL_SERVICE_VERSION` (optional)
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT` (optional; default `http://localhost:4317`)
    /// - `OTEL_LOG_FORMAT` (`pretty` or `json`; default `pretty`)
    pub fn from_env() -> Result<Self> {
        let service_name = std::env::var("OTEL_SERVICE_NAME").map_err(|_| Error::EnvConfig {
            key: "OTEL_SERVICE_NAME".into(),
            message: "not set".into(),
        })?;

        let service_version = std::env::var("OTEL_SERVICE_VERSION").ok();
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        let log_format = match std::env::var("OTEL_LOG_FORMAT").ok().as_deref() {
            Some("json") => LogFormat::Json,
            _ => LogFormat::Pretty,
        };

        Ok(Self {
            service_name,
            service_version,
            otlp_endpoint,
            resource_attributes: vec![],
            exporter: Exporter::Otlp,
            log_format,
        })
    }

    /// Wire the global tracing subscriber and `OTel` providers per this config.
    ///
    /// Must be called at most once per process — subsequent calls return
    /// [`Error::AlreadyInitialized`].
    pub fn init(self) -> Result<()> {
        crate::init::init(&self)
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    inner: Option<ConfigInner>,
}

#[derive(Debug, Default)]
struct ConfigInner {
    service_name: String,
    service_version: Option<String>,
    otlp_endpoint: Option<String>,
    resource_attributes: Vec<(String, String)>,
    exporter: Exporter,
    log_format: LogFormat,
}

impl ConfigBuilder {
    fn inner_mut(&mut self) -> &mut ConfigInner {
        self.inner.get_or_insert_with(ConfigInner::default)
    }

    /// Set the service name (required).
    #[must_use]
    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.inner_mut().service_name = name.into();
        self
    }

    /// Set the service version (optional).
    #[must_use]
    pub fn service_version(mut self, v: impl Into<String>) -> Self {
        self.inner_mut().service_version = Some(v.into());
        self
    }

    /// Set the OTLP endpoint (optional; defaults to `http://localhost:4317`).
    #[must_use]
    pub fn otlp_endpoint(mut self, e: impl Into<String>) -> Self {
        self.inner_mut().otlp_endpoint = Some(e.into());
        self
    }

    /// Add a resource attribute (repeatable).
    #[must_use]
    pub fn resource_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner_mut()
            .resource_attributes
            .push((key.into(), value.into()));
        self
    }

    /// Override the exporter backend.
    #[must_use]
    pub fn exporter(mut self, exp: Exporter) -> Self {
        self.inner_mut().exporter = exp;
        self
    }

    /// Override the stdout log format.
    #[must_use]
    pub fn log_format(mut self, f: LogFormat) -> Self {
        self.inner_mut().log_format = f;
        self
    }

    /// Build the [`Config`].
    ///
    /// # Panics
    ///
    /// Panics if `service_name` was not set.
    #[must_use]
    pub fn build(self) -> Config {
        let i = self
            .inner
            .expect("ConfigBuilder::build() called on empty builder");
        assert!(!i.service_name.is_empty(), "service_name is required");
        Config {
            service_name: i.service_name,
            service_version: i.service_version,
            otlp_endpoint: i.otlp_endpoint,
            resource_attributes: i.resource_attributes,
            exporter: i.exporter,
            log_format: i.log_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn builder_basic() {
        let c = Config::builder()
            .service_name("svc")
            .service_version("1.2.3")
            .resource_attribute("env", "test")
            .build();
        assert_eq!(c.service_name, "svc");
        assert_eq!(c.service_version, Some("1.2.3".into()));
        assert_eq!(c.resource_attributes, vec![("env".into(), "test".into())]);
    }

    #[test]
    #[should_panic(expected = "service_name is required")]
    fn build_panics_without_name() {
        let _ = Config::builder().service_version("v").build();
    }

    #[test]
    fn from_env_missing_service_name_errors() {
        // Best-effort: this test assumes OTEL_SERVICE_NAME is not set in this process.
        // If something else set it, the test is skipped.
        if std::env::var("OTEL_SERVICE_NAME").is_ok() {
            return;
        }
        let r = Config::from_env();
        assert!(matches!(r, Err(Error::EnvConfig { .. })));
    }
}
