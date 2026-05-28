//! Enable CORS, compression, and a custom request timeout.
//!
//! Run with: `cargo run --example custom_middleware -p altair-server`

use altair_server::Server;
use altair_server::axum::routing::get;
use std::time::Duration;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3002")
        .enable_cors()
        .enable_compression()
        .request_timeout(Duration::from_secs(5))
        .route(
            "/",
            get(|| async {
                // Big response to demonstrate compression in action.
                "x".repeat(1024)
            }),
        )
        .build()
        .await?
        .run()
        .await
}
