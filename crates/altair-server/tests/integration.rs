//! End-to-end behaviour tests: bind ephemeral port, hit it with reqwest,
//! verify response details.

use altair_server::axum::routing::get;
use altair_server::prelude::*;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::oneshot;

async fn start_server(builder: ServerBuilder) -> (std::net::SocketAddr, oneshot::Sender<()>) {
    let server = builder.bind_addr("127.0.0.1:0").build().await.unwrap();
    let addr = server.local_addr();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = server
            .run_with_shutdown(async move {
                let _ = rx.await;
            })
            .await;
    });
    // Give the server a moment to start accepting connections.
    tokio::time::sleep(Duration::from_millis(20)).await;
    (addr, tx)
}

#[tokio::test]
async fn default_health_endpoint_returns_200() {
    let (addr, shutdown) = start_server(Server::builder()).await;
    let response = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(response.status(), 200);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn user_route_returns_handler_body() {
    let (addr, shutdown) =
        start_server(Server::builder().route("/greet", get(|| async { "hello world" }))).await;
    let body = reqwest::get(format!("http://{addr}/greet"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "hello world");
    let _ = shutdown.send(());
}

#[tokio::test]
async fn request_id_header_is_echoed() {
    let (addr, shutdown) = start_server(Server::builder().route("/", get(|| async { "ok" }))).await;
    let response = reqwest::get(format!("http://{addr}/")).await.unwrap();
    assert!(response.headers().contains_key("x-request-id"));
    let _ = shutdown.send(());
}

#[tokio::test]
async fn timeout_returns_408() {
    let (addr, shutdown) = start_server(
        Server::builder()
            .request_timeout(Duration::from_millis(50))
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    "done"
                }),
            ),
    )
    .await;
    let response = reqwest::get(format!("http://{addr}/slow")).await.unwrap();
    assert_eq!(response.status(), 408);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn custom_health_path_works() {
    let (addr, shutdown) = start_server(Server::builder().health_path("/ready")).await;
    let response = reqwest::get(format!("http://{addr}/ready")).await.unwrap();
    assert_eq!(response.status(), 200);
    let _ = shutdown.send(());
}

#[tokio::test]
async fn disable_health_removes_endpoint() {
    let (addr, shutdown) = start_server(Server::builder().disable_health()).await;
    let response = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(response.status(), 404);
    let _ = shutdown.send(());
}
