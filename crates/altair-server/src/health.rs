//! Built-in `/health` endpoint.

use axum::Router;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::sync::Arc;

/// Closure that produces the health endpoint's response.
pub(crate) type HealthResponder = Arc<dyn Fn() -> Response + Send + Sync + 'static>;

/// Default response: 200 OK with an empty body.
pub(crate) fn default_responder() -> HealthResponder {
    Arc::new(|| ().into_response())
}

/// Register the configured health route on `router`, if enabled.
pub(crate) fn install<S>(
    router: Router<S>,
    enabled: bool,
    path: &str,
    responder: HealthResponder,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !enabled {
        return router;
    }
    let handler = move || {
        let r = responder.clone();
        async move { r() }
    };
    router.route(path, get(handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::body::Body;
    use http::Request;
    use tower::ServiceExt;

    fn build_router() -> Router {
        let router: Router = Router::new();
        install(router, true, "/health", default_responder())
    }

    #[tokio::test]
    async fn default_health_returns_200() {
        let router = build_router();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn disabled_health_returns_404() {
        let router = install(Router::new(), false, "/health", default_responder());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn custom_path_responds() {
        let router = install(Router::new(), true, "/healthz", default_responder());
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn custom_responder_runs() {
        let responder: HealthResponder = Arc::new(|| {
            (StatusCode::SERVICE_UNAVAILABLE, "db down").into_response()
        });
        let router = install(Router::new(), true, "/health", responder);
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
