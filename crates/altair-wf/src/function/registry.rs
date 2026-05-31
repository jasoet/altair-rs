//! Thread-safe `String -> Handler` registry.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use crate::error::{Error, Result};
use crate::function::payload::{FunctionInput, FunctionOutput};

/// Errors returned by a handler. Boxed so handlers can surface any
/// error type; the activity wrapper renders it via `Display`.
pub type HandlerError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Internal type alias for the boxed-pinned-future shape handlers must
/// produce. Consumers shouldn't construct this directly — use
/// [`Registry::register`] which wraps any `async fn` matching the
/// signature.
type BoxedFuture =
    Pin<Box<dyn Future<Output = std::result::Result<FunctionOutput, HandlerError>> + Send>>;

/// Erased handler type stored in the registry's map.
type StoredHandler = Arc<dyn Fn(FunctionInput) -> BoxedFuture + Send + Sync + 'static>;

/// Thread-safe registry of named handlers.
///
/// A handler is any `async fn(FunctionInput) -> Result<FunctionOutput, E>`
/// where `E` boxes into an `std::error::Error`. Handlers are stored
/// behind an `Arc<dyn Fn ...>`, so cloning the registry is cheap and
/// concurrent dispatch is safe.
///
/// Cloning the registry returns a new handle pointing at the same
/// inner `RwLock<HashMap>` — registering on one handle is visible to
/// every other clone, mirroring the Go original's pointer semantics.
#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<RwLock<HashMap<String, StoredHandler>>>,
}

impl Registry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `handler` under `name`. Returns
    /// [`Error::InvalidInput`] when a handler with the same name is
    /// already present.
    ///
    /// The handler signature is `Fn(FunctionInput) -> Future<Result<FunctionOutput, E>>`
    /// where `E` boxes into a `std::error::Error`. Any `async fn` you
    /// declare with that shape compiles.
    pub fn register<F, Fut, E>(&mut self, name: impl Into<String>, handler: F) -> Result<()>
    where
        F: Fn(FunctionInput) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<FunctionOutput, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let name = name.into();
        let mut map = self.inner.write().expect("registry RwLock poisoned");
        if map.contains_key(&name) {
            return Err(Error::InvalidInput(format!(
                "handler already registered: {name}"
            )));
        }
        let wrapped: StoredHandler = Arc::new(move |input| {
            let fut = handler(input);
            Box::pin(async move {
                fut.await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>)
            })
        });
        map.insert(name, wrapped);
        Ok(())
    }

    /// `true` if a handler with `name` is registered.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.inner
            .read()
            .expect("registry RwLock poisoned")
            .contains_key(name)
    }

    /// Borrow the handler for `name`. Returns
    /// [`Error::InvalidInput`] if no handler is registered.
    pub fn get(&self, name: &str) -> Result<StoredHandler> {
        self.inner
            .read()
            .expect("registry RwLock poisoned")
            .get(name)
            .cloned()
            .ok_or_else(|| Error::InvalidInput(format!("function {name:?} not found in registry")))
    }

    /// Number of registered handlers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().expect("registry RwLock poisoned").len()
    }

    /// `true` if no handlers are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Look the handler up by name and run it with `input`. Sugar over
    /// `get(name)?.call(input).await`.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidInput`] if no handler is registered for `name`.
    /// - [`Error::PatternStopped`] if the handler returns an error
    ///   (the boxed error message is preserved in the `reason` field).
    pub async fn dispatch(&self, name: &str, input: FunctionInput) -> Result<FunctionOutput> {
        let handler = self.get(name)?;
        handler(input).await.map_err(|e| Error::PatternStopped {
            position: name.to_string(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_then_dispatch_returns_handler_output() {
        let mut reg = Registry::new();
        reg.register("upper", |input: FunctionInput| async move {
            let value = input.args.get("text").cloned().unwrap_or_default();
            Ok::<_, std::io::Error>(FunctionOutput::with_result([(
                "out".to_string(),
                value.to_uppercase(),
            )]))
        })
        .unwrap();

        let out = reg
            .dispatch("upper", FunctionInput::with_args([("text", "hi")]))
            .await
            .unwrap();
        assert_eq!(out.result.get("out").unwrap(), "HI");
    }

    #[tokio::test]
    async fn duplicate_registration_is_rejected() {
        let mut reg = Registry::new();
        reg.register("a", |_| async {
            Ok::<_, std::io::Error>(FunctionOutput::default())
        })
        .unwrap();
        let res = reg.register("a", |_| async {
            Ok::<_, std::io::Error>(FunctionOutput::default())
        });
        assert!(matches!(res, Err(Error::InvalidInput(_))));
    }

    #[tokio::test]
    async fn missing_handler_errors_on_dispatch() {
        let reg = Registry::new();
        let res = reg.dispatch("ghost", FunctionInput::default()).await;
        assert!(matches!(res, Err(Error::InvalidInput(_))));
    }

    #[tokio::test]
    async fn handler_error_maps_to_pattern_stopped() {
        let mut reg = Registry::new();
        reg.register("boom", |_input| async move {
            Err::<FunctionOutput, _>(std::io::Error::other("kaboom"))
        })
        .unwrap();
        let err = reg
            .dispatch("boom", FunctionInput::default())
            .await
            .unwrap_err();
        match err {
            Error::PatternStopped { position, reason } => {
                assert_eq!(position, "boom");
                assert!(reason.contains("kaboom"));
            }
            other => panic!("expected PatternStopped, got {other:?}"),
        }
    }

    #[test]
    fn has_and_len_track_registrations() {
        let mut reg = Registry::new();
        assert!(reg.is_empty());
        assert!(!reg.has("x"));
        reg.register("x", |_| async {
            Ok::<_, std::io::Error>(FunctionOutput::default())
        })
        .unwrap();
        assert_eq!(reg.len(), 1);
        assert!(reg.has("x"));
    }

    #[tokio::test]
    async fn clone_sees_registrations_on_original() {
        let mut a = Registry::new();
        let b = a.clone();
        a.register("k", |_| async {
            Ok::<_, std::io::Error>(FunctionOutput::default())
        })
        .unwrap();
        // The cloned handle shares the same backing storage.
        assert!(b.has("k"));
    }
}
