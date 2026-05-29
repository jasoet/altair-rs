//! Pure mapping from `Config` to `sea_orm::ConnectOptions`.

use sea_orm::ConnectOptions;

use crate::config::Config;

/// Translate a `Config` into a `sea_orm::ConnectOptions`.
///
/// Pure function — does not open a connection. Centralises the
/// "every Config knob maps to the right ConnectOptions field" logic so it
/// can be unit-tested without a live database.
pub(crate) fn build_options(config: &Config) -> ConnectOptions {
    let mut opt = ConnectOptions::new(config.url.clone());
    opt.max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .sqlx_logging(config.sqlx_logging)
        .sqlx_slow_statements_logging_settings(
            log::LevelFilter::Warn,
            config.sqlx_slow_query_threshold,
        );
    if let Some(d) = config.idle_timeout {
        opt.idle_timeout(d);
    }
    if let Some(d) = config.max_lifetime {
        opt.max_lifetime(d);
    }
    opt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample_config() -> Config {
        Config {
            url: "sqlite::memory:".to_string(),
            max_connections: 25,
            min_connections: 3,
            acquire_timeout: Duration::from_secs(7),
            idle_timeout: Some(Duration::from_secs(120)),
            max_lifetime: Some(Duration::from_secs(900)),
            sqlx_logging: false,
            sqlx_slow_query_threshold: Duration::from_millis(250),
        }
    }

    #[test]
    fn maps_url() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_url(), "sqlite::memory:");
    }

    #[test]
    fn maps_pool_sizes() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_max_connections(), Some(25));
        assert_eq!(opts.get_min_connections(), Some(3));
    }

    #[test]
    fn maps_timeouts() {
        let opts = build_options(&sample_config());
        assert_eq!(opts.get_acquire_timeout(), Some(Duration::from_secs(7)));
        assert_eq!(opts.get_idle_timeout(), Some(Duration::from_secs(120)));
        assert_eq!(opts.get_max_lifetime(), Some(Duration::from_secs(900)));
    }

    #[test]
    fn maps_logging_flag() {
        let opts = build_options(&sample_config());
        assert!(!opts.get_sqlx_logging());
    }

    #[test]
    fn optional_timeouts_default_to_none_when_unset() {
        let mut cfg = sample_config();
        cfg.idle_timeout = None;
        cfg.max_lifetime = None;
        let opts = build_options(&cfg);
        assert_eq!(opts.get_idle_timeout(), None);
        assert_eq!(opts.get_max_lifetime(), None);
    }
}
