//! End-to-end behaviour tests: bind ephemeral port, hit it with reqwest,
//! verify response details.

#![allow(
    tail_expr_drop_order,
    clippy::duration_suboptimal_units,
    clippy::let_underscore_future
)]

use altair_server::axum;
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

#[tokio::test]
async fn concurrent_requests_all_succeed() {
    let (addr, shutdown) = start_server(Server::builder().route(
        "/echo",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            "ok"
        }),
    ))
    .await;

    let url = format!("http://{addr}/echo");
    let client = reqwest::Client::new();
    let handles: Vec<_> = (0..32)
        .map(|_| {
            let client = client.clone();
            let url = url.clone();
            tokio::spawn(async move { client.get(&url).send().await.unwrap().status().as_u16() })
        })
        .collect();

    for h in handles {
        assert_eq!(h.await.unwrap(), 200);
    }
    let _ = shutdown.send(());
}

#[tokio::test]
async fn oversized_body_returns_413() {
    let (addr, shutdown) = start_server(
        Server::builder()
            .request_body_limit(1024) // 1 KiB cap
            .route(
                "/upload",
                axum::routing::post(
                    |body: axum::body::Bytes| async move { body.len().to_string() },
                ),
            ),
    )
    .await;

    // 2 KiB payload, twice the limit.
    let payload = vec![b'x'; 2048];
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/upload"))
        .body(payload)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        413,
        "body exceeding limit must be rejected"
    );
    let _ = shutdown.send(());
}

#[tokio::test]
async fn body_within_limit_passes_through() {
    let (addr, shutdown) = start_server(
        Server::builder()
            .request_body_limit(4096) // 4 KiB cap
            .route(
                "/upload",
                axum::routing::post(
                    |body: axum::body::Bytes| async move { body.len().to_string() },
                ),
            ),
    )
    .await;

    let payload = vec![b'a'; 1024];
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/upload"))
        .body(payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "1024");
    let _ = shutdown.send(());
}

#[tokio::test]
async fn shutdown_drains_in_flight_request() {
    // Slow handler that returns 200 after 200ms. Trigger shutdown while
    // the request is mid-flight and verify the response still arrives
    // OK (drain semantics) rather than being abruptly cut off.
    let server = Server::builder()
        .bind_addr("127.0.0.1:0")
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(200)).await;
                "drained"
            }),
        )
        .build()
        .await
        .unwrap();
    let addr = server.local_addr();

    let (tx, rx) = oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        server
            .run_with_shutdown(async move {
                let _ = rx.await;
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = reqwest::Client::new();
    let request = client.get(format!("http://{addr}/slow")).send();
    let request_handle = tokio::spawn(request);

    // Signal shutdown after the request is in-flight but before the handler returns.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = tx.send(());

    let body = request_handle.await.unwrap().unwrap().text().await.unwrap();
    assert_eq!(
        body, "drained",
        "in-flight request should drain successfully"
    );

    // Server should exit Ok within reasonable time.
    let res = tokio::time::timeout(Duration::from_secs(5), serve)
        .await
        .expect("server exits within 5s")
        .expect("join task");
    assert!(res.is_ok(), "server should exit cleanly: {res:?}");
}

#[tokio::test]
async fn shutdown_timeout_fires_when_handler_hangs() {
    let server = Server::builder()
        .bind_addr("127.0.0.1:0")
        .shutdown_timeout(Duration::from_millis(100))
        .route(
            "/forever",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                "never"
            }),
        )
        .build()
        .await
        .unwrap();
    let addr = server.local_addr();

    let (tx, rx) = oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        server
            .run_with_shutdown(async move {
                let _ = rx.await;
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = reqwest::Client::new();
    // Fire-and-forget the slow request; we don't await it.
    let _ = tokio::spawn(async move {
        let _ = client.get(format!("http://{addr}/forever")).send().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = tx.send(());

    let res = tokio::time::timeout(Duration::from_secs(5), serve)
        .await
        .expect("server exits within 5s")
        .expect("join task");
    assert!(
        matches!(res, Err(altair_server::Error::ShutdownTimeout(_))),
        "expected ShutdownTimeout, got {res:?}",
    );
}
