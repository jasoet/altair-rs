//! Cross-crate auto-integration with `altair-otel`.
//!
//! `altair-server` includes `tower_http::trace::TraceLayer` by default, so
//! every incoming HTTP request emits a span with `http.method`, `http.target`,
//! `http.status_code`, etc. When `altair-otel` is initialised in the same
//! process, those spans flow to the configured exporter — no extra wiring
//! required.
//!
//! This example uses the Stdout exporter so no collector is needed. Once
//! running, hit it with:
//!
//!   `curl http://127.0.0.1:3030/`
//!   `curl http://127.0.0.1:3030/echo/hello`
//!
//! Spans will print to the server's stdout. Press Ctrl-C to exit.
//!
//! Run with: `cargo run --example with_otel -p altair-server`

use altair_otel::{Config as OtelConfig, Exporter};
use altair_server::Server;
use altair_server::axum::extract::Path;
use altair_server::axum::routing::get;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("server-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let server = Server::builder()
        .bind_addr("127.0.0.1:3030")
        .route("/", get(|| async { "hello from altair-server" }))
        .route(
            "/echo/{name}",
            get(|Path(name): Path<String>| async move { format!("hello {name}") }),
        )
        .build()
        .await?;

    println!("listening on {}", server.local_addr());
    println!("try: curl http://{}/", server.local_addr());
    println!("     curl http://{}/echo/world", server.local_addr());
    println!("Press Ctrl-C to shut down");

    let result = server.run().await;

    // Drain pending spans before exit.
    altair_otel::shutdown();
    result.map_err(Into::into)
}
