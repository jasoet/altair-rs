//! Crate-wide error type for `altair-rest`.

use thiserror::Error;

/// Errors returned by `altair-rest` operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Middleware-chain failure — the typical retry-exhausted / network-error
    /// path. Wraps a `reqwest_middleware::Error`.
    #[error("HTTP request failed: {0}")]
    Middleware(#[from] reqwest_middleware::Error),

    /// Raw `reqwest::Error` — surfaces when a path doesn't flow through the
    /// middleware stack (e.g. `error_for_status` results after middleware
    /// has already returned the response).
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),

    /// Response body failed to deserialize as the requested type
    /// (`get_json` / `post_json`).
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    /// Bad URL — typically from [`crate::ClientBuilder::base_url`] or
    /// from relative-path resolution at request time.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// Invalid HTTP header name or value (from
    /// [`crate::ClientBuilder::default_header`]).
    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_error_renders() {
        let json_err: serde_json::Error = serde_json::from_str::<u32>("not a number").unwrap_err();
        let e: Error = json_err.into();
        assert!(e.to_string().starts_with("decode error:"));
    }

    #[test]
    fn url_error_renders() {
        let url_err: url::ParseError = "not a url".parse::<url::Url>().unwrap_err();
        let e: Error = url_err.into();
        assert!(e.to_string().starts_with("invalid URL:"));
    }

    #[test]
    fn invalid_header_renders() {
        let e = Error::InvalidHeader("name contains spaces".into());
        assert_eq!(e.to_string(), "invalid header: name contains spaces");
    }
}
