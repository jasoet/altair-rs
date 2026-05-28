//! `Server` — the constructed runtime that binds, serves, and shuts down.

use crate::builder::ServerBuilder;
use crate::error::{Error, Result};
use crate::shutdown::shutdown_signal;
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Configured server, bound to a TCP listener.
///
/// Build via [`Server::builder`].
pub struct Server {
    router: Router<()>,
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl Server {
    /// Start building a new server.
    #[must_use]
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Internal constructor used by [`ServerBuilder::build`].
    pub(crate) fn from_parts(
        router: Router<()>,
        listener: TcpListener,
        local_addr: SocketAddr,
    ) -> Self {
        Self {
            router,
            listener,
            local_addr,
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
    pub async fn run_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        axum::serve(self.listener, self.router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(Error::from)
    }
}
