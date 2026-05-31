//! [`Mapper`] trait and convenience implementations: [`IdentityMapper`]
//! when source and sink share a type, [`RecordMapper`] for per-record
//! conversion with skip tolerance.

use std::marker::PhantomData;

use async_trait::async_trait;

use crate::datasync::result::MapResult;
use crate::error::Result;

/// Transforms a batch of source records (type `T`) into a batch of sink
/// records (type `U`). The framework hands the entire fetched batch to a
/// single `map` call.
#[async_trait]
pub trait Mapper<T, U>: Send + Sync
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Transform the input batch. Implementations that fail the whole
    /// batch on any per-record error should return `Err`; implementations
    /// that prefer to drop bad records should implement
    /// [`DetailedMapper`] instead and return [`MapResult::skipped`].
    async fn map(&self, records: Vec<T>) -> Result<Vec<U>>;
}

/// Sibling trait of [`Mapper`] that exposes a synchronous,
/// skip-tracking variant. Not a supertrait — implementations of
/// `DetailedMapper` typically also `impl Mapper<T, U>` separately
/// (see [`RecordMapper`] for the reference shape). A blanket impl
/// (`impl<M: DetailedMapper> Mapper for M`) would create coherence
/// issues for downstream types that need both, so the duplication is
/// intentional.
pub trait DetailedMapper<T, U>: Send + Sync
where
    T: Send + 'static,
    U: Send + 'static,
{
    /// Synchronously transform the batch, recording each skip with a
    /// reason. Used by [`RecordMapper`].
    fn map_detailed(&self, records: Vec<T>) -> MapResult<U>;
}

/// No-op mapper: passes records through unchanged. Use when source and
/// sink agree on the record type.
pub struct IdentityMapper<T> {
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Default for IdentityMapper<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> IdentityMapper<T> {
    /// Build a new identity mapper.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl<T> Mapper<T, T> for IdentityMapper<T>
where
    T: Send + Sync + 'static,
{
    async fn map(&self, records: Vec<T>) -> Result<Vec<T>> {
        Ok(records)
    }
}

/// Per-record mapper: applies `fn(record) -> Result<U>` to each input
/// independently. Per-record errors are recorded in [`MapResult::skipped`]
/// rather than failing the whole batch — useful when occasional bad
/// records should not stop the sync.
///
/// `F` is `Fn(&T) -> Result<U, E>` where `E` satisfies
/// `std::error::Error + Send + Sync + 'static`; the error message is
/// captured in `skip_reasons` via `Display`. The bound matches the one
/// the `function` module's registry uses for handler errors so user
/// error types compose between the two features without conversion.
pub struct RecordMapper<T, U, F, E>
where
    F: Fn(&T) -> std::result::Result<U, E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
    T: Send + Sync + 'static,
    U: Send + 'static,
{
    name: String,
    func: F,
    _phantom: PhantomData<fn(T) -> (U, E)>,
}

impl<T, U, F, E> RecordMapper<T, U, F, E>
where
    F: Fn(&T) -> std::result::Result<U, E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
    T: Send + Sync + 'static,
    U: Send + 'static,
{
    /// Build a `RecordMapper` with a stable `name` for logs/traces and a
    /// per-record conversion function.
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self {
            name: name.into(),
            func,
            _phantom: PhantomData,
        }
    }

    /// The mapper's stable identifier.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T, U, F, E> DetailedMapper<T, U> for RecordMapper<T, U, F, E>
where
    F: Fn(&T) -> std::result::Result<U, E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
    T: Send + Sync + 'static,
    U: Send + 'static,
{
    fn map_detailed(&self, records: Vec<T>) -> MapResult<U> {
        let mut out = MapResult {
            records: Vec::with_capacity(records.len()),
            skipped: 0,
            skip_reasons: Vec::new(),
        };
        for (i, record) in records.iter().enumerate() {
            match (self.func)(record) {
                Ok(u) => out.records.push(u),
                Err(e) => {
                    out.skipped += 1;
                    out.skip_reasons.push(format!("record {i}: {e}"));
                }
            }
        }
        out
    }
}

#[async_trait]
impl<T, U, F, E> Mapper<T, U> for RecordMapper<T, U, F, E>
where
    F: Fn(&T) -> std::result::Result<U, E> + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
    T: Send + Sync + 'static,
    U: Send + 'static,
{
    async fn map(&self, records: Vec<T>) -> Result<Vec<U>> {
        Ok(self.map_detailed(records).records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identity_mapper_passes_records_through() {
        let m: IdentityMapper<i32> = IdentityMapper::new();
        let out = m.map(vec![1, 2, 3]).await.unwrap();
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn record_mapper_collects_skip_reasons() {
        let m = RecordMapper::new(
            "doubler",
            |x: &i32| -> std::result::Result<i32, std::io::Error> {
                if *x % 2 == 0 {
                    Ok(x * 2)
                } else {
                    Err(std::io::Error::other(format!("odd value {x}")))
                }
            },
        );
        let detail = m.map_detailed(vec![1, 2, 3, 4]);
        assert_eq!(detail.records, vec![4, 8]);
        assert_eq!(detail.skipped, 2);
        assert_eq!(detail.skip_reasons.len(), 2);
        assert!(detail.skip_reasons[0].contains("odd value 1"));
    }

    #[tokio::test]
    async fn record_mapper_via_mapper_trait_drops_skips() {
        let m = RecordMapper::new(
            "doubler",
            |x: &i32| -> std::result::Result<i32, std::io::Error> {
                if *x % 2 == 0 {
                    Ok(x * 2)
                } else {
                    Err(std::io::Error::other("odd"))
                }
            },
        );
        let out = m.map(vec![1, 2, 3, 4]).await.unwrap();
        assert_eq!(out, vec![4, 8]);
    }

    #[tokio::test]
    async fn record_mapper_happy_path_preserves_all_records() {
        // Regression: when every record maps cleanly, `Mapper::map` must
        // return them in order with zero skips — pins the no-skip branch
        // of `DetailedMapper::map_detailed`.
        let m = RecordMapper::new(
            "doubler",
            |x: &i32| -> std::result::Result<i32, std::io::Error> { Ok(x * 2) },
        );
        let detail = m.map_detailed(vec![1, 2, 3]);
        assert_eq!(detail.records, vec![2, 4, 6]);
        assert_eq!(detail.skipped, 0);
        assert!(detail.skip_reasons.is_empty());

        let mapped = m.map(vec![10, 20, 30]).await.unwrap();
        assert_eq!(mapped, vec![20, 40, 60]);
    }

    #[test]
    fn record_mapper_name_accessor() {
        let m: RecordMapper<i32, i32, _, std::io::Error> =
            RecordMapper::new("doubler", |x: &i32| Ok(x * 2));
        assert_eq!(m.name(), "doubler");
    }
}
