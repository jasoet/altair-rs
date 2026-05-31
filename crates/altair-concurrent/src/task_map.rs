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
    ///
    /// # Duplicate names
    ///
    /// Backed by `BTreeMap::insert` — calling `insert` twice with the
    /// same `name` **silently overwrites** the earlier task ("last write
    /// wins"). If you need duplicate-name detection, check `.len()`
    /// before/after the second insert or use distinct names.
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

    #[test]
    fn default_is_empty() {
        let m: TaskMap<u32> = TaskMap::default();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn len_after_three_inserts() {
        let m: TaskMap<u32> = TaskMap::new()
            .insert("a", |_| async { Ok::<_, std::io::Error>(1) })
            .insert("b", |_| async { Ok::<_, std::io::Error>(2) })
            .insert("c", |_| async { Ok::<_, std::io::Error>(3) });
        assert_eq!(m.len(), 3);
        assert!(!m.is_empty());
    }

    #[tokio::test]
    async fn task_closure_executes_after_insert() {
        let m: TaskMap<u32> =
            TaskMap::new().insert("only", |_| async { Ok::<_, std::io::Error>(99) });
        assert_eq!(m.len(), 1);
        // Pull the task out and run it manually to exercise the boxed closure path.
        let (_name, task_fn) = m.tasks.into_iter().next().unwrap();
        let ct = tokio_util::sync::CancellationToken::new();
        let out = task_fn(ct).await.unwrap();
        assert_eq!(out, 99);
    }
}
