//! Default middleware-stack assembly.
//!
//! Order applied (outermost → innermost):
//!
//! 1. `TraceLayer::new_for_http()` — `OTel`-aware request span
//! 2. `SetRequestIdLayer::x_request_id(MakeRequestUuid)` — assign UUID if missing
//! 3. `PropagateRequestIdLayer::x_request_id()` — echo ID in response
//! 4. `CorsLayer` (if enabled)
//! 5. `CompressionLayer` (if enabled)
//! 6. `RequestBodyLimitLayer` — reject oversized bodies
//! 7. `TimeoutLayer::new(timeout)` — request deadline
//!
//! User-added layers via `with_middleware` are applied innermost-of-stack
//! (closest to the handler), which gives them visibility into the
//! request-id and trace context.

use axum::Router;
use axum::http::StatusCode;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

/// Configuration for the default middleware stack.
pub(crate) struct DefaultStack {
    pub tracing: bool,
    pub request_id: bool,
    pub timeout: Duration,
    pub body_limit: usize,
    pub cors: Option<CorsLayer>,
    pub compression: bool,
}

impl DefaultStack {
    /// Apply the configured layers to a router.
    pub(crate) fn apply<S>(self, router: Router<S>) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut router = router;

        // Innermost first because Router::layer wraps outwards.
        router = router.layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            self.timeout,
        ));

        router = router.layer(RequestBodyLimitLayer::new(self.body_limit));

        if self.compression {
            router = router.layer(CompressionLayer::new());
        }

        if let Some(cors) = self.cors {
            router = router.layer(cors);
        }

        if self.request_id {
            router = router
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));
        }

        if self.tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    fn router_with_stack(stack: DefaultStack) -> Router {
        let base: Router = Router::new().route("/", get(|| async { "ok" }));
        stack.apply(base)
    }

    #[tokio::test]
    async fn defaults_pass_through_ok_request() {
        let stack = DefaultStack {
            tracing: true,
            request_id: true,
            timeout: Duration::from_secs(5),
            body_limit: 2 * 1024 * 1024,
            cors: None,
            compression: false,
        };
        let router = router_with_stack(stack);
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn request_id_disabled_omits_header() {
        let stack = DefaultStack {
            tracing: false,
            request_id: false,
            timeout: Duration::from_secs(5),
            body_limit: 2 * 1024 * 1024,
            cors: None,
            compression: false,
        };
        let router = router_with_stack(stack);
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn timeout_returns_408_when_slow_handler() {
        let stack = DefaultStack {
            tracing: false,
            request_id: false,
            timeout: Duration::from_millis(20),
            body_limit: 2 * 1024 * 1024,
            cors: None,
            compression: false,
        };
        let base: Router = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                "done"
            }),
        );
        let router = stack.apply(base);
        let response = router
            .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }
}
