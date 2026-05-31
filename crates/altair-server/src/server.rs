//! `Server` — the constructed runtime that binds, serves, and shuts down.

use crate::builder::ServerBuilder;
use crate::error::{Error, Result};
use crate::shutdown::shutdown_signal;
use axum::Router;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;

/// Configured server, bound to a TCP listener.
///
/// Build via [`Server::builder`].
pub struct Server {
    router: Router<()>,
    listener: TcpListener,
    local_addr: SocketAddr,
    shutdown_timeout: Option<Duration>,
}

impl Server {
    /// Start building a new server.
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Internal constructor used by [`ServerBuilder::build`].
    pub(crate) fn from_parts(
        router: Router<()>,
        listener: TcpListener,
        local_addr: SocketAddr,
        shutdown_timeout: Option<Duration>,
    ) -> Self {
        Self {
            router,
            listener,
            local_addr,
            shutdown_timeout,
        }
    }

    /// Actual bound socket address.
    ///
    /// Useful when `bind_addr("0.0.0.0:0")` was used so the OS chose a port.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Bind the listener and serve forever (until SIGINT / SIGTERM).
    ///
    /// Returns `Ok(())` after graceful shutdown completes.
    pub async fn run(self) -> Result<()> {
        self.run_with_shutdown(shutdown_signal()).await
    }

    /// Bind and serve until the given future resolves.
    ///
    /// If [`ServerBuilder::shutdown_timeout`] was set, the post-shutdown
    /// drain is bounded — in-flight requests still running after the
    /// deadline cause this method to return
    /// [`Error::ShutdownTimeout`] instead of waiting forever.
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let timeout = self.shutdown_timeout;
        let serve = axum::serve(self.listener, self.router).with_graceful_shutdown(shutdown);
        match timeout {
            Some(d) => match tokio::time::timeout(d, serve).await {
                Ok(res) => res.map_err(Error::from),
                Err(_) => Err(Error::ShutdownTimeout(d)),
            },
            None => serve.await.map_err(Error::from),
        }
    }
}
