//! End-to-end behaviour tests using wiremock as an in-process HTTP server.

#![allow(tail_expr_drop_order)]

use altair_rest::prelude::*;
use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct User {
    id: u64,
    name: String,
}

#[tokio::test]
async fn get_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(User {
            id: 1,
            name: "alice".into(),
        }))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let user: User = client.get_json("/users/1").await.unwrap();
    assert_eq!(
        user,
        User {
            id: 1,
            name: "alice".into()
        }
    );
}

#[tokio::test]
async fn post_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users"))
        .respond_with(ResponseTemplate::new(201).set_body_json(User {
            id: 2,
            name: "bob".into(),
        }))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let new_user = User {
        id: 0,
        name: "bob".into(),
    };
    let created: User = client.post_json("/users", &new_user).await.unwrap();
    assert_eq!(created.id, 2);
}

#[tokio::test]
async fn retries_on_5xx_then_succeeds() {
    struct Flaky {
        counter: Arc<AtomicU32>,
    }
    impl Respond for Flaky {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 3 {
                ResponseTemplate::new(503)
            } else {
                ResponseTemplate::new(200).set_body_string("ok")
            }
        }
    }

    let counter = Arc::new(AtomicU32::new(0));
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(Flaky {
            counter: counter.clone(),
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .retry_initial_interval(Duration::from_millis(10))
        .retry_max_interval(Duration::from_millis(50))
        .build()
        .unwrap();

    let response = client.get("/flaky").send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn does_not_retry_on_400() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bad"))
        .respond_with(move |_: &wiremock::Request| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(400)
        })
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .retry_initial_interval(Duration::from_millis(10))
        .build()
        .unwrap();

    let response = client.get("/bad").send().await.unwrap();
    assert_eq!(response.status(), 400);
    // 400 is a client error — should not be retried.
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn bearer_token_is_applied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/secure"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer my-secret-token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .bearer_token("my-secret-token")
        .build()
        .unwrap();

    let response = client.get("/secure").send().await.unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn put_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(User {
            id: 1,
            name: "alice".into(),
        }))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let updated: User = client
        .put_json(
            "/users/1",
            &User {
                id: 1,
                name: "alice".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "alice");
}

#[tokio::test]
async fn patch_json_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(User {
            id: 1,
            name: "alicia".into(),
        }))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let patched: User = client
        .patch_json("/users/1", &serde_json::json!({ "name": "alicia" }))
        .await
        .unwrap();
    assert_eq!(patched.name, "alicia");
}

#[tokio::test]
async fn get_json_rejects_oversized_response() {
    let server = MockServer::start().await;
    // 2 KiB body, but client caps at 512 bytes.
    let big = serde_json::json!({"payload": "x".repeat(2048)});
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_json(big))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .response_body_limit(512)
        .build()
        .unwrap();

    let err = client
        .get_json::<serde_json::Value>("/big")
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::ResponseTooLarge { .. }),
        "expected ResponseTooLarge, got {err:?}",
    );
}

#[tokio::test]
async fn max_redirects_zero_rejects_redirect() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/redir"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/elsewhere"))
        .mount(&server)
        .await;
    // /elsewhere never matches because the client must not follow.

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .max_redirects(0)
        .build()
        .unwrap();

    let response = client.get("/redir").send().await.unwrap();
    assert_eq!(response.status(), 302, "should not follow when max=0");
}

#[tokio::test]
async fn delete_request_works() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/users/1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let response = client.delete("/users/1").send().await.unwrap();
    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn concurrent_requests_succeed_independently() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ping"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .build()
        .unwrap();

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let c = client.clone();
            tokio::spawn(async move { c.get("/ping").send().await.map(|r| r.status().as_u16()) })
        })
        .collect();

    for h in handles {
        assert_eq!(h.await.unwrap().unwrap(), 200);
    }
}

#[tokio::test]
async fn default_headers_are_applied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/with-header"))
        .and(wiremock::matchers::header("x-tenant", "acme"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .base_url(&server.uri())
        .unwrap()
        .default_header("x-tenant", "acme")
        .unwrap()
        .build()
        .unwrap();

    let response = client.get("/with-header").send().await.unwrap();
    assert_eq!(response.status(), 200);
}
