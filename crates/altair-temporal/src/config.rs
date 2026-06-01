//! Configuration types for altair-temporal.

use std::path::PathBuf;
use std::time::Duration;

/// Connection + worker configuration.
///
/// All fields have defaults except `task_queue`, which is required.
///
/// # Concurrency vs rate fields
///
/// - [`max_concurrent_activities`](Self::max_concurrent_activities): the
///   true concurrency cap — how many activity executions can be in
///   flight on the worker at once. Wired into the SDK's slot supplier.
///   Defaults to `100`.
/// - [`max_concurrent_workflows`](Self::max_concurrent_workflows): same
///   shape, but for workflow tasks. Defaults to `100`.
/// - [`max_cached_workflows`](Self::max_cached_workflows): the size of
///   the worker's sticky-cache LRU — distinct from concurrency.
///   Defaults to `1000`.
///
/// Earlier versions of this crate mis-wired `max_concurrent_activities`
/// to the SDK's `max_worker_activities_per_second` rate limit, which
/// silently throttled prod workers to N executions per second
/// regardless of how many could run in parallel. The fields now mean
/// what their names say.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Temporal server URL, e.g. `https://temporal.prod.internal:7233`.
    pub host: String,
    /// Temporal namespace.
    pub namespace: String,
    /// Task queue this worker polls / this client targets.
    pub task_queue: String,
    /// Worker / client identity (visible in the Temporal UI).
    ///
    /// `None` (the default) lets the SDK pick `"<pid>@<hostname>"` —
    /// the correct choice for production so each replica is
    /// distinguishable in the Temporal worker view. Set explicitly only
    /// for tests or single-worker setups.
    pub identity: Option<String>,
    /// Maximum **concurrent** activities the worker may execute
    /// simultaneously (slot capacity, not a rate limit).
    pub max_concurrent_activities: u32,
    /// Maximum **concurrent** workflow tasks the worker may execute
    /// simultaneously (slot capacity, not the sticky cache size — see
    /// [`max_cached_workflows`](Self::max_cached_workflows)).
    pub max_concurrent_workflows: u32,
    /// Size of the workflow sticky-cache LRU. Higher = fewer cache
    /// misses on repeat workflow executions; lower = less memory.
    /// Defaults to `1000` to match the SDK default.
    pub max_cached_workflows: usize,
    /// How long the worker waits for in-flight activities to finish
    /// after [`Worker::run_with_shutdown`]'s shutdown future resolves.
    /// Activities exceeding this period are cancelled with no further
    /// notice. Set to `Duration::ZERO` (the SDK default) to disable
    /// the graceful drain.
    ///
    /// Defaults to **30 seconds** — long enough to finish most
    /// well-behaved activities, short enough that SIGTERM still
    /// completes inside the typical Kubernetes
    /// `terminationGracePeriodSeconds` default of 30s.
    ///
    /// [`Worker::run_with_shutdown`]: crate::Worker::run_with_shutdown
    #[serde(with = "duration_secs")]
    pub shutdown_grace: Duration,
    /// Optional TLS configuration. `None` = plaintext (local dev).
    pub tls: Option<TlsConfig>,
}

/// TLS configuration for the gRPC connection to Temporal.
///
/// `Debug` is implemented manually to redact paths in case future
/// additions (auth tokens, API keys) land here — the convention is to
/// keep the field types but obscure their values in log output.
#[derive(Clone, serde::Deserialize)]
pub struct TlsConfig {
    /// Path to the server's CA certificate (PEM). `None` falls back
    /// to system trust roots — the right setting for Temporal Cloud
    /// and any internal CA that's already in the OS trust store.
    pub server_root_ca_cert: Option<PathBuf>,
    /// Optional client certificate (PEM) for mutual TLS.
    pub client_cert: Option<PathBuf>,
    /// Optional client key (PEM) for mutual TLS.
    pub client_key: Option<PathBuf>,
    /// Optional gRPC SNI / server-name override.
    pub server_name_override: Option<String>,
}

impl std::fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Redact path values; show only presence/absence. Future
        // secrets (API keys etc.) will fall back to this same shape.
        f.debug_struct("TlsConfig")
            .field(
                "server_root_ca_cert",
                &self.server_root_ca_cert.as_ref().map(|_| "<set>"),
            )
            .field("client_cert", &self.client_cert.as_ref().map(|_| "<set>"))
            .field("client_key", &self.client_key.as_ref().map(|_| "<set>"))
            .field("server_name_override", &self.server_name_override)
            .finish()
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Duration, D::Error> {
        let secs = u64::deserialize(de)?;
        Ok(Duration::from_secs(secs))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "http://localhost:7233".to_string(),
            namespace: "default".to_string(),
            task_queue: String::new(),
            identity: None,
            max_concurrent_activities: 100,
            max_concurrent_workflows: 100,
            max_cached_workflows: 1000,
            shutdown_grace: Duration::from_secs(30),
            tls: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(c.host, "http://localhost:7233");
        assert_eq!(c.namespace, "default");
        assert_eq!(c.task_queue, "");
        assert!(c.identity.is_none());
        assert_eq!(c.max_concurrent_activities, 100);
        assert_eq!(c.max_concurrent_workflows, 100);
        assert_eq!(c.max_cached_workflows, 1000);
        assert_eq!(c.shutdown_grace, Duration::from_secs(30));
        assert!(c.tls.is_none());
    }

    #[test]
    fn deserialise_minimal_toml() {
        let toml_src = r#"
task_queue = "demo"
"#;
        let c: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(c.task_queue, "demo");
        assert_eq!(c.host, "http://localhost:7233"); // default kicks in
    }

    #[test]
    fn deserialise_full_toml() {
        let toml_src = r#"
host = "https://temporal.prod.example:7233"
namespace = "archive"
task_queue = "archive-tq"
identity = "archive-rs-worker"
max_concurrent_activities = 50
max_concurrent_workflows = 50
max_cached_workflows = 500
shutdown_grace = 15

[tls]
server_root_ca_cert = "/etc/temporal/ca.pem"
client_cert = "/etc/temporal/client.crt"
client_key = "/etc/temporal/client.key"
server_name_override = "temporal.internal"
"#;
        let c: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(c.namespace, "archive");
        assert_eq!(c.max_concurrent_activities, 50);
        assert_eq!(c.max_cached_workflows, 500);
        assert_eq!(c.shutdown_grace, Duration::from_secs(15));
        assert_eq!(c.identity.as_deref(), Some("archive-rs-worker"));
        let tls = c.tls.expect("tls");
        assert_eq!(
            tls.server_root_ca_cert.as_ref().unwrap().to_str().unwrap(),
            "/etc/temporal/ca.pem"
        );
        assert_eq!(
            tls.server_name_override.as_deref(),
            Some("temporal.internal")
        );
    }

    #[test]
    fn tls_debug_redacts_paths() {
        let tls = TlsConfig {
            server_root_ca_cert: Some(PathBuf::from("/etc/temporal/ca.pem")),
            client_cert: Some(PathBuf::from("/etc/temporal/client.crt")),
            client_key: Some(PathBuf::from("/etc/temporal/client.key")),
            server_name_override: Some("temporal.internal".into()),
        };
        let s = format!("{tls:?}");
        // Path values do not appear in Debug output; the marker does.
        assert!(!s.contains("/etc/temporal"));
        assert!(s.contains("<set>"));
        // Non-secret fields still appear.
        assert!(s.contains("temporal.internal"));
    }
}
