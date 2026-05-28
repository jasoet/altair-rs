//! Typed builder for [`crate::Server`].

use crate::error::{Error, Result};
use crate::health::{self, HealthResponder};
use crate::middleware::DefaultStack;
use crate::server::Server;
use axum::Router;
use axum::handler::Handler;
use axum::response::IntoResponse;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_HEALTH_PATH: &str = "/health";

/// Typed builder for [`Server`].
///
/// Construct via [`Server::builder`](crate::Server::builder).
#[must_use]
pub struct ServerBuilder {
    bind_addr: String,
    router: Router<()>,
    tracing: bool,
    request_id: bool,
    timeout: Duration,
    cors: Option<CorsLayer>,
    compression: bool,
    health_enabled: bool,
    health_path: String,
    health_responder: HealthResponder,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
            router: Router::new(),
            tracing: true,
            request_id: true,
            timeout: DEFAULT_TIMEOUT,
            cors: None,
            compression: false,
            health_enabled: true,
            health_path: DEFAULT_HEALTH_PATH.to_string(),
            health_responder: health::default_responder(),
        }
    }
}

impl ServerBuilder {
    /// Create a builder with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bind address as a string (e.g. `"0.0.0.0:3000"`, `"[::]:8080"`).
    pub fn bind_addr(mut self, addr: impl Into<String>) -> Self {
        self.bind_addr = addr.into();
        self
    }

    /// Set the bind address as a parsed `SocketAddr`.
    pub fn bind_socket(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr.to_string();
        self
    }

    /// Register a route, delegating to [`axum::Router::route`].
    pub fn route<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, axum::routing::any(handler));
        self
    }

    /// Merge another router (delegates to [`axum::Router::merge`]).
    pub fn merge(mut self, other: Router) -> Self {
        self.router = self.router.merge(other);
        self
    }

    /// Mount a router at a nested path (delegates to [`axum::Router::nest`]).
    pub fn nest(mut self, prefix: &str, router: Router) -> Self {
        self.router = self.router.nest(prefix, router);
        self
    }

    /// Set the per-request timeout. Default 30s.
    pub fn request_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Disable the default tracing middleware.
    pub fn disable_tracing(mut self) -> Self {
        self.tracing = false;
        self
    }

    /// Disable the default request-id middleware.
    pub fn disable_request_id(mut self) -> Self {
        self.request_id = false;
        self
    }

    /// Enable CORS with permissive defaults (`CorsLayer::permissive()`).
    pub fn enable_cors(mut self) -> Self {
        self.cors = Some(CorsLayer::permissive());
        self
    }

    /// Enable CORS with a custom [`CorsLayer`].
    pub fn enable_cors_with(mut self, layer: CorsLayer) -> Self {
        self.cors = Some(layer);
        self
    }

    /// Enable response compression (gzip/br/zstd).
    pub fn enable_compression(mut self) -> Self {
        self.compression = true;
        self
    }

    /// Customise the health endpoint path. Default `/health`.
    pub fn health_path(mut self, path: &str) -> Self {
        self.health_path = path.to_string();
        self
    }

    /// Provide a custom responder for the health endpoint.
    pub fn health_response<F, R>(mut self, responder: F) -> Self
    where
        F: Fn() -> R + Send + Sync + 'static,
        R: IntoResponse + 'static,
    {
        self.health_responder = Arc::new(move || responder().into_response());
        self
    }

    /// Disable the built-in health endpoint.
    pub fn disable_health(mut self) -> Self {
        self.health_enabled = false;
        self
    }

    /// Bind the listener and build a [`Server`] ready to run.
    pub async fn build(self) -> Result<Server> {
        let addr: SocketAddr = self
            .bind_addr
            .parse()
            .map_err(|e| Error::Configuration(format!("invalid bind address '{}': {e}", self.bind_addr)))?;

        let listener = TcpListener::bind(addr).await.map_err(|e| Error::Bind {
            addr: self.bind_addr.clone(),
            source: e,
        })?;
        let local_addr = listener.local_addr().map_err(Error::from)?;

        // Register health first so it always wins on its configured path.
        let router = health::install(
            self.router,
            self.health_enabled,
            &self.health_path,
            self.health_responder,
        );

        let stack = DefaultStack {
            tracing: self.tracing,
            request_id: self.request_id,
            timeout: self.timeout,
            cors: self.cors,
            compression: self.compression,
        };

        let router = stack.apply(router);

        Ok(Server::from_parts(router, listener, local_addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_with_defaults_binds_ephemeral_port() {
        let server = ServerBuilder::new()
            .bind_addr("127.0.0.1:0")
            .build()
            .await
            .unwrap();
        let addr = server.local_addr();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn build_rejects_invalid_bind_address() {
        let result = ServerBuilder::new()
            .bind_addr("not a socket address")
            .build()
            .await;
        assert!(matches!(result, Err(Error::Configuration(_))));
    }

    #[tokio::test]
    async fn build_with_bind_socket_works() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = ServerBuilder::new()
            .bind_socket(addr)
            .build()
            .await
            .unwrap();
        assert_eq!(server.local_addr().ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn build_with_custom_timeout() {
        let server = ServerBuilder::new()
            .bind_addr("127.0.0.1:0")
            .request_timeout(Duration::from_secs(5))
            .build()
            .await
            .unwrap();
        let _ = server.local_addr();
    }
}
