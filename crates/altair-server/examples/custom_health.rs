//! Customize the health endpoint path and response body.
//!
//! Run with: `cargo run --example custom_health -p altair-server`
//!
//! Hit it: `curl http://127.0.0.1:3005/readyz`

use altair_server::Server;
use altair_server::axum::Json;
use serde_json::json;

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    Server::builder()
        .bind_addr("127.0.0.1:3005")
        .health_path("/readyz")
        .health_response(|| {
            Json(json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        })
        .build()
        .await?
        .run()
        .await
}
