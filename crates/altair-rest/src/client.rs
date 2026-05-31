//! `Client` newtype around `reqwest_middleware::ClientWithMiddleware`.

use crate::config::ClientBuilder;
use crate::error::{Error, Result};
use reqwest_middleware::{ClientWithMiddleware, RequestBuilder};
use url::Url;

/// HTTP client with retry + tracing middleware baked in.
///
/// Construct via [`Client::builder`]. The client is cheap to clone and
/// uses an internal connection pool — share one instance across your app.
#[derive(Clone)]
pub struct Client {
    inner: ClientWithMiddleware,
    base_url: Option<Url>,
    bearer_token: Option<String>,
    basic_auth: Option<(String, Option<String>)>,
    pub(crate) response_body_limit: u64,
}

impl Client {
    /// Start building a new client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Internal constructor used by [`ClientBuilder::build`].
    pub(crate) fn from_parts(
        inner: ClientWithMiddleware,
        base_url: Option<Url>,
        bearer_token: Option<String>,
        basic_auth: Option<(String, Option<String>)>,
        response_body_limit: u64,
    ) -> Self {
        Self {
            inner,
            base_url,
            bearer_token,
            basic_auth,
            response_body_limit,
        }
    }

    /// Access the underlying `reqwest_middleware::ClientWithMiddleware`.
    /// Use this if you need to call into the library directly.
    #[must_use]
    pub fn inner(&self) -> &ClientWithMiddleware {
        &self.inner
    }

    /// Resolve a relative or absolute URL against the optional base.
    pub(crate) fn resolve_url(&self, url: &str) -> Result<Url> {
        if let Some(base) = &self.base_url {
            base.join(url).map_err(Error::from)
        } else {
            Url::parse(url).map_err(Error::from)
        }
    }

    fn prepare(&self, builder: RequestBuilder) -> RequestBuilder {
        let mut builder = builder;
        if let Some(token) = &self.bearer_token {
            builder = builder.bearer_auth(token);
        }
        if let Some((user, password)) = &self.basic_auth {
            builder = builder.basic_auth(user, password.as_deref());
        }
        builder
    }

    /// Build a GET request.
    pub fn get(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.get(u)),
            Err(_) => self.prepare(self.inner.get(url)),
        }
    }

    /// Build a POST request.
    pub fn post(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.post(u)),
            Err(_) => self.prepare(self.inner.post(url)),
        }
    }

    /// Build a PUT request.
    pub fn put(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.put(u)),
            Err(_) => self.prepare(self.inner.put(url)),
        }
    }

    /// Build a DELETE request.
    pub fn delete(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.delete(u)),
            Err(_) => self.prepare(self.inner.delete(url)),
        }
    }

    /// Build a PATCH request.
    pub fn patch(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.patch(u)),
            Err(_) => self.prepare(self.inner.patch(url)),
        }
    }

    /// Build a HEAD request.
    pub fn head(&self, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.head(u)),
            Err(_) => self.prepare(self.inner.head(url)),
        }
    }

    /// Build a request with a custom method.
    pub fn request(&self, method: reqwest::Method, url: &str) -> RequestBuilder {
        match self.resolve_url(url) {
            Ok(u) => self.prepare(self.inner.request(method, u)),
            Err(_) => self.prepare(self.inner.request(method, url)),
        }
    }

    /// Execute a pre-built request.
    pub async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response> {
        self.inner.execute(request).await.map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> Client {
        Client::builder()
            .base_url("https://api.example.com/v1/")
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn resolve_url_joins_relative_path() {
        let c = test_client();
        let url = c.resolve_url("users/42").unwrap();
        assert_eq!(url.as_str(), "https://api.example.com/v1/users/42");
    }

    #[test]
    fn resolve_url_absolute_overrides_base() {
        let c = test_client();
        let url = c.resolve_url("https://other.example.com/path").unwrap();
        assert_eq!(url.as_str(), "https://other.example.com/path");
    }

    #[test]
    fn resolve_url_without_base_requires_absolute() {
        let c = Client::builder().build().unwrap();
        assert!(c.resolve_url("https://example.com/x").is_ok());
        assert!(c.resolve_url("/x").is_err());
    }

    #[tokio::test]
    async fn client_is_clone() {
        let c1 = Client::builder().build().unwrap();
        let _c2 = c1.clone();
    }
}
