//! Helpers for working with Temporal `ActivityError`.

use temporalio_sdk::activities::ActivityError;

/// Convert an error into an [`ActivityError::application`] failure,
/// marking it `non_retryable` when `is_permanent(&err)` returns `true`.
///
/// The error's `Display` is used as the failure message; the type name
/// (from `std::any::type_name::<E>()`) is used as the failure `type` so
/// it can be matched in `RetryPolicy::non_retryable(...)` lists.
pub fn classify_error<E, F>(err: E, is_permanent: F) -> ActivityError
where
    E: std::error::Error + Send + Sync + 'static,
    F: FnOnce(&E) -> bool,
{
    let permanent = is_permanent(&err);
    let type_name = std::any::type_name::<E>().to_string();
    let failure = temporalio_common::error::ApplicationFailure::builder(err)
        .type_name(type_name)
        .non_retryable(permanent)
        .build();
    ActivityError::application(failure)
}
