//! Configuration types for altair-db.

use std::time::Duration;

use crate::error::{Error, Result};

/// Which database backend a `Config` refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// `PostgreSQL`.
    Postgres,
    /// `MySQL` / `MariaDB`.
    MySql,
    /// `SQLite`.
    Sqlite,
}

/// Connection + pool configuration for `Db`.
///
/// Field defaults: `max_connections=10`, `min_connections=1`,
/// `acquire_timeout=30s`, `idle_timeout=10m`, `max_lifetime=30m`,
/// `sqlx_logging=true`, `sqlx_slow_query_threshold=1s`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Connection URL, e.g. `postgres://user:pass@host/db`.
    pub url: String,
    /// Maximum number of pooled connections.
    pub max_connections: u32,
    /// Minimum number of pooled connections kept warm.
    pub min_connections: u32,
    /// How long to wait for a connection from the pool before failing.
    #[serde(with = "humantime_serde")]
    pub acquire_timeout: Duration,
    /// Close idle connections after this duration (`None` = never).
    #[serde(with = "humantime_serde", default)]
    pub idle_timeout: Option<Duration>,
    /// Recycle connections older than this (`None` = never).
    #[serde(with = "humantime_serde", default)]
    pub max_lifetime: Option<Duration>,
    /// Whether sqlx should emit `tracing` events for every query.
    pub sqlx_logging: bool,
    /// Statements slower than this are logged at WARN level.
    #[serde(with = "humantime_serde")]
    pub sqlx_slow_query_threshold: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_mins(10)),
            max_lifetime: Some(Duration::from_mins(30)),
            sqlx_logging: true,
            sqlx_slow_query_threshold: Duration::from_secs(1),
        }
    }
}

impl Config {
    /// Build a `Config` with default pool tuning and the given URL.
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }

    /// Parse the URL scheme into a `Backend`.
    ///
    /// Errors with `Error::Configuration` if the URL is empty or the scheme
    /// is not one of `postgres://`, `postgresql://`, `mysql://`, `sqlite://`.
    pub fn backend(&self) -> Result<Backend> {
        let url = self.url.trim();
        if url.is_empty() {
            return Err(Error::Configuration("url is required".to_string()));
        }
        let scheme = url
            .split_once(':')
            .map_or(url, |(s, _)| s)
            .to_ascii_lowercase();
        match scheme.as_str() {
            "postgres" | "postgresql" => Ok(Backend::Postgres),
            "mysql" => Ok(Backend::MySql),
            "sqlite" => Ok(Backend::Sqlite),
            other => Err(Error::Configuration(format!(
                "unsupported url scheme: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_url_keeps_defaults() {
        let c = Config::from_url("postgres://localhost/x");
        assert_eq!(c.url, "postgres://localhost/x");
        assert_eq!(c.max_connections, 10);
        assert_eq!(c.acquire_timeout, Duration::from_secs(30));
        assert_eq!(c.idle_timeout, Some(Duration::from_mins(10)));
    }

    #[test]
    fn backend_postgres() {
        assert_eq!(
            Config::from_url("postgres://x/y").backend().unwrap(),
            Backend::Postgres
        );
        assert_eq!(
            Config::from_url("postgresql://x/y").backend().unwrap(),
            Backend::Postgres
        );
    }

    #[test]
    fn backend_mysql() {
        assert_eq!(
            Config::from_url("mysql://x/y").backend().unwrap(),
            Backend::MySql
        );
    }

    #[test]
    fn backend_sqlite() {
        assert_eq!(
            Config::from_url("sqlite::memory:").backend().unwrap(),
            Backend::Sqlite
        );
        assert_eq!(
            Config::from_url("sqlite:///tmp/x.db").backend().unwrap(),
            Backend::Sqlite
        );
    }

    #[test]
    fn backend_rejects_unknown_scheme() {
        let err = Config::from_url("mongodb://x").backend().unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn backend_rejects_empty_url() {
        let err = Config::default().backend().unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
    }

    #[test]
    fn humantime_round_trip() {
        let toml_src = r#"
url = "postgres://localhost/x"
acquire_timeout = "45s"
idle_timeout = "5m"
max_lifetime = "1h"
sqlx_slow_query_threshold = "750ms"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.acquire_timeout, Duration::from_secs(45));
        assert_eq!(cfg.idle_timeout, Some(Duration::from_mins(5)));
        assert_eq!(cfg.max_lifetime, Some(Duration::from_hours(1)));
        assert_eq!(
            cfg.sqlx_slow_query_threshold,
            Duration::from_millis(750)
        );
    }
}
