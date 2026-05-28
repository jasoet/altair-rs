//! Hook into shutdown explicitly via a channel. Useful for tests and
//! orchestration where you need to trigger shutdown programmatically
//! (not via SIGINT).
//!
//! Run with: `cargo run --example graceful_shutdown -p altair-server`
//!
//! This example shuts itself down after 5 seconds.

use altair_server::Server;
use altair_server::axum::routing::get;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let server = Server::builder()
        .bind_addr("127.0.0.1:3003")
        .route("/", get(|| async { "still running" }))
        .build()
        .await?;

    println!("listening on {}", server.local_addr());

    server
        .run_with_shutdown(async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            println!("triggering shutdown");
        })
        .await
}
