//! Typed `ClientBuilder` and the middleware-chain construction.

use crate::client::Client;
use crate::error::{Error, Result};
use http::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest_middleware::{ClientBuilder as MiddlewareBuilder, Middleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::TracingMiddleware;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RETRY_INITIAL: Duration = Duration::from_millis(100);
const DEFAULT_RETRY_MAX: Duration = Duration::from_secs(5);
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_USER_AGENT: &str = concat!("altair-rest/", env!("CARGO_PKG_VERSION"));

/// Typed builder for [`Client`].
///
/// Construct via [`Client::builder`].
#[must_use]
pub struct ClientBuilder {
    base_url: Option<Url>,
    timeout: Duration,
    connect_timeout: Duration,
    user_agent: String,
    headers: HeaderMap,
    bearer_token: Option<String>,
    basic_auth: Option<(String, Option<String>)>,
    retry_max_attempts: u32,
    retry_initial_interval: Duration,
    retry_max_interval: Duration,
    enable_tracing: bool,
    reqwest_customize:
        Option<Box<dyn FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder + Send>>,
    extra_middleware: Vec<Arc<dyn Middleware>>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        Self {
            base_url: None,
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            headers,
            bearer_token: None,
            basic_auth: None,
            retry_max_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_initial_interval: DEFAULT_RETRY_INITIAL,
            retry_max_interval: DEFAULT_RETRY_MAX,
            enable_tracing: true,
            reqwest_customize: None,
            extra_middleware: Vec::new(),
        }
    }
}

impl ClientBuilder {
    /// Create a builder with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base URL for relative path resolution. Must end with a `/` if
    /// you want subpaths to nest underneath; e.g. `"https://api.example.com/v1/"`.
    pub fn base_url(mut self, url: &str) -> Result<Self> {
        let parsed = Url::parse(url)?;
        self.base_url = Some(parsed);
        Ok(self)
    }

    /// Total request timeout.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// TCP connect timeout.
    pub fn connect_timeout(mut self, d: Duration) -> Self {
        self.connect_timeout = d;
        self
    }

    /// Override the `User-Agent` header. Default: `altair-rest/<version>`.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        let ua_string = ua.into();
        self.user_agent.clone_from(&ua_string);
        if let Ok(value) = HeaderValue::from_str(&ua_string) {
            self.headers.insert(USER_AGENT, value);
        }
        self
    }

    /// Add a default header applied to every request.
    pub fn default_header(mut self, name: &str, value: &str) -> Result<Self> {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| Error::InvalidHeader(format!("name '{name}': {e}")))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|e| Error::InvalidHeader(format!("value: {e}")))?;
        self.headers.insert(header_name, header_value);
        Ok(self)
    }

    /// Set a Bearer token. Equivalent to `default_header("authorization", "Bearer <token>")`.
    pub fn bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Set HTTP Basic auth credentials. Applied to every request.
    pub fn basic_auth(mut self, user: impl Into<String>, password: Option<&str>) -> Self {
        self.basic_auth = Some((user.into(), password.map(str::to_string)));
        self
    }

    /// Maximum number of retry attempts. `0` disables retries entirely.
    pub fn retry_max_attempts(mut self, n: u32) -> Self {
        self.retry_max_attempts = n;
        self
    }

    /// Initial retry backoff interval.
    pub fn retry_initial_interval(mut self, d: Duration) -> Self {
        self.retry_initial_interval = d;
        self
    }

    /// Maximum retry backoff interval (caps exponential growth).
    pub fn retry_max_interval(mut self, d: Duration) -> Self {
        self.retry_max_interval = d;
        self
    }

    /// Toggle the built-in tracing middleware.
    pub fn enable_tracing(mut self, on: bool) -> Self {
        self.enable_tracing = on;
        self
    }

    /// Escape hatch: customize the underlying `reqwest::ClientBuilder`.
    pub fn with_reqwest_builder<F>(mut self, customize: F) -> Self
    where
        F: FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder + Send + 'static,
    {
        self.reqwest_customize = Some(Box::new(customize));
        self
    }

    /// Escape hatch: append a custom middleware to the chain. Custom middleware
    /// runs AFTER the built-in retry and tracing middleware.
    pub fn with_middleware<M>(mut self, middleware: M) -> Self
    where
        M: Middleware + 'static,
    {
        self.extra_middleware.push(Arc::new(middleware));
        self
    }

    /// Build the configured [`Client`].
    pub fn build(self) -> Result<Client> {
        // 1) Construct the inner reqwest client.
        let mut reqwest_builder = reqwest::ClientBuilder::new()
            .timeout(self.timeout)
            .connect_timeout(self.connect_timeout)
            .default_headers(self.headers.clone());

        if let Some(customize) = self.reqwest_customize {
            reqwest_builder = customize(reqwest_builder);
        }

        let reqwest_client = reqwest_builder.build()?;

        // 2) Assemble the middleware chain.
        let mut chain = MiddlewareBuilder::new(reqwest_client);

        if self.retry_max_attempts > 0 {
            let policy = ExponentialBackoff::builder()
                .retry_bounds(self.retry_initial_interval, self.retry_max_interval)
                .build_with_max_retries(self.retry_max_attempts);
            chain = chain.with(RetryTransientMiddleware::new_with_policy(policy));
        }

        if self.enable_tracing {
            chain = chain.with(TracingMiddleware::default());
        }

        for middleware in self.extra_middleware {
            chain = chain.with_arc(middleware);
        }

        let inner = chain.build();

        Ok(Client::from_parts(
            inner,
            self.base_url,
            self.bearer_token,
            self.basic_auth,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_user_agent_has_crate_version() {
        let cb = ClientBuilder::default();
        let ua = cb.headers.get(USER_AGENT).unwrap().to_str().unwrap();
        assert!(ua.starts_with("altair-rest/"));
    }

    #[test]
    fn base_url_parsing_accepts_valid_url() {
        let cb = ClientBuilder::new()
            .base_url("https://api.example.com/")
            .unwrap();
        assert!(cb.base_url.is_some());
    }

    #[test]
    fn base_url_rejects_invalid_url() {
        let res = ClientBuilder::new().base_url("not a url");
        assert!(matches!(res, Err(Error::Url(_))));
    }

    #[test]
    fn default_header_rejects_invalid_name() {
        let res = ClientBuilder::new().default_header("name with space", "v");
        assert!(matches!(res, Err(Error::InvalidHeader(_))));
    }

    #[test]
    fn default_header_rejects_invalid_value() {
        let res = ClientBuilder::new().default_header("x-custom", "value\nwith\nnewlines");
        assert!(matches!(res, Err(Error::InvalidHeader(_))));
    }

    #[test]
    fn retry_zero_attempts_builds_without_retry_middleware() {
        let client = ClientBuilder::new().retry_max_attempts(0).build();
        assert!(client.is_ok());
    }

    #[test]
    fn bearer_token_is_stored() {
        let cb = ClientBuilder::new().bearer_token("xyz");
        assert_eq!(cb.bearer_token.as_deref(), Some("xyz"));
    }

    #[test]
    fn build_default_succeeds() {
        let client = ClientBuilder::new().build();
        assert!(client.is_ok());
    }
}
