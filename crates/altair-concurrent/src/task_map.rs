//! Builder for a set of named concurrent tasks.

use futures::future::BoxFuture;
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

type BoxedTaskFn<T> =
    Box<dyn FnOnce(CancellationToken) -> BoxFuture<'static, Result<T, BoxedError>> + Send>;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// A set of named tasks to run concurrently.
///
/// `T` is the success result type — all tasks in a `TaskMap` produce the
/// same `T`. For heterogeneous batches, use `tokio::join!` directly.
pub struct TaskMap<T> {
    pub(crate) tasks: BTreeMap<&'static str, BoxedTaskFn<T>>,
}

impl<T> TaskMap<T> {
    /// Create an empty task map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }

    /// Insert a named task into the map.
    ///
    /// The closure receives the active [`CancellationToken`] and must return
    /// a future producing `Result<T, E>` where `E` can be boxed into a
    /// `std::error::Error`.
    #[must_use]
    pub fn insert<F, Fut, E>(mut self, name: &'static str, task: F) -> Self
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = std::result::Result<T, E>> + Send + 'static,
        E: Into<BoxedError>,
        T: Send + 'static,
    {
        let boxed: BoxedTaskFn<T> = Box::new(move |token| {
            let fut = task(token);
            Box::pin(async move { fut.await.map_err(Into::into) })
        });
        self.tasks.insert(name, boxed);
        self
    }

    /// Return the number of tasks currently in the map.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Return `true` if no tasks have been inserted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl<T> Default for TaskMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_is_empty() {
        let m: TaskMap<u32> = TaskMap::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn insert_increments_len() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("b", |_| async { Ok::<_, std::io::Error>(2) });
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn insert_duplicate_overwrites() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("a", |_| async { Ok::<_, std::io::Error>(2) });
        assert_eq!(m.len(), 1);
    }
}
