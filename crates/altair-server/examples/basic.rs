//! Smallest possible altair-server: one route, defaults.
//!
//! Run with: `cargo run --example basic -p altair-server`
//!
//! Hit it: `curl http://127.0.0.1:3000/`

use altair_server::Server;
use altair_server::axum::routing::get;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3000")
        .route("/", get(|| async { "hello from altair-server" }))
        .build()
        .await?
        .run()
        .await
}
