//! Configuration types for altair-temporal.

use std::path::PathBuf;

/// Connection + worker configuration.
///
/// All fields have defaults except `task_queue`, which is required.
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
    pub identity: String,
    /// Maximum concurrent activities the worker may execute.
    pub max_concurrent_activities: u32,
    /// Maximum concurrent workflow tasks the worker may execute.
    pub max_concurrent_workflows: u32,
    /// Optional TLS configuration. `None` = plaintext (local dev).
    pub tls: Option<TlsConfig>,
}

/// TLS configuration for the gRPC connection to Temporal.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TlsConfig {
    /// Path to the server's CA certificate (PEM).
    pub server_root_ca_cert: PathBuf,
    /// Optional client certificate (PEM) for mutual TLS.
    pub client_cert: Option<PathBuf>,
    /// Optional client key (PEM) for mutual TLS.
    pub client_key: Option<PathBuf>,
    /// Optional gRPC SNI / server-name override.
    pub server_name_override: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "http://localhost:7233".to_string(),
            namespace: "default".to_string(),
            task_queue: String::new(),
            identity: "altair-temporal".to_string(),
            max_concurrent_activities: 100,
            max_concurrent_workflows: 100,
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
        assert_eq!(c.identity, "altair-temporal");
        assert_eq!(c.max_concurrent_activities, 100);
        assert_eq!(c.max_concurrent_workflows, 100);
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

[tls]
server_root_ca_cert = "/etc/temporal/ca.pem"
client_cert = "/etc/temporal/client.crt"
client_key = "/etc/temporal/client.key"
server_name_override = "temporal.internal"
"#;
        let c: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(c.namespace, "archive");
        assert_eq!(c.max_concurrent_activities, 50);
        let tls = c.tls.expect("tls");
        assert_eq!(tls.server_root_ca_cert.to_str().unwrap(), "/etc/temporal/ca.pem");
        assert_eq!(tls.server_name_override.as_deref(), Some("temporal.internal"));
    }
}
