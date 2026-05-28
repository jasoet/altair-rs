//! Shared application state via axum's `Router::with_state`.
//!
//! `ServerBuilder` doesn't expose a typed state knob (the generic-S
//! signature is awkward). Instead, build an `axum::Router<()>` with state
//! already applied (via `Router::with_state`) and pass it to `.merge()`.
//!
//! Run with: `cargo run --example with_state -p altair-server`

use altair_server::Server;
use altair_server::axum::Router;
use altair_server::axum::extract::State;
use altair_server::axum::routing::get;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    greeting: Arc<String>,
}

async fn greet(State(state): State<AppState>) -> String {
    format!("{} from altair-server", state.greeting)
}

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let state = AppState {
        greeting: Arc::new("hello".to_string()),
    };

    let router: Router = Router::new().route("/greet", get(greet)).with_state(state);

    Server::builder()
        .bind_addr("127.0.0.1:3004")
        .merge(router)
        .build()
        .await?
        .run()
        .await
}
