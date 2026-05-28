//! Multiple routes, nested router, JSON response.
//!
//! Run with: `cargo run --example with_routes -p altair-server`

use altair_server::Server;
use altair_server::axum::Json;
use altair_server::axum::Router;
use altair_server::axum::routing::get;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

async fn list_users() -> Json<Vec<User>> {
    Json(vec![User {
        id: 1,
        name: "alice".into(),
    }])
}

async fn create_user(Json(payload): Json<CreateUser>) -> Json<User> {
    Json(User {
        id: 42,
        name: payload.name,
    })
}

#[tokio::main]
async fn main() -> altair_server::Result<()> {
    let api: Router = Router::new().route("/users", get(list_users).post(create_user));

    Server::builder()
        .bind_addr("127.0.0.1:3001")
        .route("/", get(|| async { "hello" }))
        .nest("/api", api)
        .build()
        .await?
        .run()
        .await
}
