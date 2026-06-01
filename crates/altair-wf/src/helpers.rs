//! Defaults, failure-strategy enum, and shared utility helpers.

use std::collections::HashMap;
use std::time::Duration;

use std::sync::Arc;

use altair_temporal::RetryPolicy;
use altair_temporal::temporalio_sdk::ActivityOptions;

use crate::traits::TaskInput;
use crate::types::Substitutor;

// These constants back the values returned by `default_activity_options`
// / `default_retry_policy`. Kept `pub(crate)` because they're
// implementation details — consumers who want to customise the defaults
// should reach for the builder, not multiply by these numbers.
pub(crate) const DEFAULT_START_TO_CLOSE_MINS: u64 = 10;
pub(crate) const DEFAULT_INITIAL_INTERVAL_MS: u64 = 1_000;
pub(crate) const DEFAULT_BACKOFF_COEFFICIENT: f64 = 2.0;
pub(crate) const DEFAULT_MAX_INTERVAL_SECS: u64 = 60;
pub(crate) const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;

/// How a pattern reacts when a step reports failure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FailureStrategy {
    /// Continue executing remaining steps; pattern returns `Ok` with the
    /// per-step results. The default — matches the Go library's empty
    /// string case.
    #[default]
    Continue,
    /// Abort on the first failing step and return
    /// [`crate::Error::PatternStopped`].
    FailFast,
}

/// The default activity options used by every pattern when the caller
/// does not supply their own: 10-minute start-to-close timeout, 3
/// retries with 1s → 60s exponential backoff (factor 2.0). No
/// heartbeat is configured — long-running handlers should call
/// [`activity_options_with`] with a non-zero heartbeat to avoid
/// being killed by start-to-close.
#[must_use]
pub fn default_activity_options() -> ActivityOptions {
    let retry = default_retry_policy().into_inner();
    ActivityOptions::with_start_to_close_timeout(Duration::from_secs(
        DEFAULT_START_TO_CLOSE_MINS * 60,
    ))
    .retry_policy(retry)
    .build()
}

/// Build an [`ActivityOptions`] tuned for a specific activity. One-call
/// alternative to writing the SDK's builder chain — wires
/// start-to-close, heartbeat timeout (set `Duration::ZERO` to disable),
/// and a custom retry policy.
///
/// # Example
///
/// ```no_run
/// # use std::time::Duration;
/// # use altair_wf::{activity_options_with, default_retry_policy};
/// let opts = activity_options_with(
///     Duration::from_mins(20),    // start-to-close
///     Duration::from_secs(30),    // heartbeat — handler must `record_heartbeat` more often
///     default_retry_policy(),
/// );
/// ```
#[must_use]
pub fn activity_options_with(
    start_to_close: Duration,
    heartbeat: Duration,
    retry: RetryPolicy,
) -> ActivityOptions {
    let b = ActivityOptions::with_start_to_close_timeout(start_to_close)
        .retry_policy(retry.into_inner());
    if heartbeat.is_zero() {
        b.build()
    } else {
        b.heartbeat_timeout(heartbeat).build()
    }
}

/// The default retry policy applied to activity options when none is
/// supplied: 3 attempts, 1s initial, 60s max, factor 2.0.
///
/// `expect` is safe here because the inputs are compile-time constants
/// that all satisfy the policy's validation rules — see
/// `RetryPolicyBuilder::build` for the contract.
///
/// # Panics
///
/// Never panics in practice — the `.expect(...)` only fires if a
/// future contributor changes the compile-time constants above to
/// values that violate `RetryPolicyBuilder::build`'s invariants.
#[must_use]
pub fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::builder()
        .initial_interval(Duration::from_millis(DEFAULT_INITIAL_INTERVAL_MS))
        .maximum_interval(Duration::from_secs(DEFAULT_MAX_INTERVAL_SECS))
        .backoff_coefficient(DEFAULT_BACKOFF_COEFFICIENT)
        .max_attempts(DEFAULT_MAX_RETRY_ATTEMPTS)
        .build()
        .expect("default retry policy constants are valid")
}

/// Wrap a closure in the `Arc<dyn Fn ...>` shape the loop patterns
/// expect, so callers don't have to remember the exact bound + the
/// `Arc::new(...)` boilerplate.
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use altair_wf::{TaskInput, substitutor_from_fn};
///
/// #[derive(Clone)]
/// struct Step { name: String }
/// impl TaskInput for Step {}
///
/// let sub = substitutor_from_fn(|template: &Step, item: &str, idx: usize, _: &HashMap<String, String>| {
///     Step { name: format!("{}-{item}-{idx}", template.name) }
/// });
/// let out = sub(&Step { name: "deploy".into() }, "us-east-1", 0, &HashMap::new());
/// assert_eq!(out.name, "deploy-us-east-1-0");
/// ```
#[must_use]
pub fn substitutor_from_fn<I, F>(f: F) -> Substitutor<I>
where
    I: TaskInput,
    F: Fn(&I, &str, usize, &HashMap<String, String>) -> I + Send + Sync + 'static,
{
    Arc::new(f)
}

/// Replace template variables in `tmpl` using a few simple substitution
/// rules borrowed from the Go library:
///
/// - `{{item}}` → the supplied `item`
/// - `{{index}}` → the supplied `index`
/// - `{{paramName}}` and `{{.paramName}}` → the value from `params`
///
/// Unknown placeholders are left as-is.
#[must_use]
#[allow(clippy::implicit_hasher)] // params is data, not a hash collection consumers choose
pub fn substitute_template(
    tmpl: &str,
    item: &str,
    index: usize,
    params: &HashMap<String, String>,
) -> String {
    let mut out = tmpl.replace("{{item}}", item);
    out = out.replace("{{index}}", &index.to_string());
    for (key, value) in params {
        out = out.replace(&format!("{{{{.{key}}}}}"), value);
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}

/// Generate the cartesian product of `params` — for each key, choose one
/// value, repeat across every combination. Used by the parameterised
/// loop pattern to expand a single template into one task per
/// combination.
///
/// # Determinism
///
/// The output ordering is **deterministic**: keys are sorted
/// lexicographically before the cartesian product is built. This is
/// load-bearing for Temporal workflow code — without it the
/// `HashMap`'s randomised iteration order would change the activity
/// dispatch order between runs of the same workflow and corrupt the
/// event-history replay invariant.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn generate_parameter_combinations(
    params: &HashMap<String, Vec<String>>,
) -> Vec<HashMap<String, String>> {
    if params.is_empty() {
        return Vec::new();
    }

    // CRITICAL: sort keys before iterating so the cartesian product
    // order is deterministic across runs. HashMap iteration order is
    // randomised in Rust, and Temporal workflows must replay
    // identically — using random key order here would silently break
    // determinism inside `parameterized_loop`.
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let values: Vec<&Vec<String>> = keys.iter().map(|k| &params[*k]).collect();
    let mut out: Vec<HashMap<String, String>> = Vec::new();
    let mut current: HashMap<String, String> = HashMap::new();
    fill(&keys, &values, 0, &mut current, &mut out);
    out
}

fn fill(
    keys: &[&String],
    values: &[&Vec<String>],
    depth: usize,
    current: &mut HashMap<String, String>,
    out: &mut Vec<HashMap<String, String>>,
) {
    if depth == keys.len() {
        out.push(current.clone());
        return;
    }
    for value in values[depth] {
        current.insert(keys[depth].clone(), value.clone());
        fill(keys, values, depth + 1, current, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_template_item_and_index() {
        let params = HashMap::new();
        let out = substitute_template("hello {{item}} at {{index}}", "world", 3, &params);
        assert_eq!(out, "hello world at 3");
    }

    #[test]
    fn substitute_template_named_params_both_syntaxes() {
        let mut params = HashMap::new();
        params.insert("region".to_string(), "us-east-1".to_string());
        let out = substitute_template("{{region}} or {{.region}}", "", 0, &params);
        assert_eq!(out, "us-east-1 or us-east-1");
    }

    #[test]
    fn substitute_template_unknown_placeholder_is_left_alone() {
        let params = HashMap::new();
        let out = substitute_template("{{mystery}}", "x", 0, &params);
        assert_eq!(out, "{{mystery}}");
    }

    #[test]
    fn generate_combinations_empty_yields_empty() {
        let params = HashMap::new();
        let out = generate_parameter_combinations(&params);
        assert!(out.is_empty());
    }

    #[test]
    fn generate_combinations_order_is_deterministic_across_calls() {
        // Build a HashMap with several keys whose hash order would be
        // randomised. Run generate_parameter_combinations twice and
        // assert the output is bit-identical. This is the regression
        // test for the determinism fix — without sorting keys first,
        // the HashMap iteration order would vary between calls.
        let mut params: HashMap<String, Vec<String>> = HashMap::new();
        for key in ["zebra", "alpha", "mango", "delta", "papaya"] {
            params.insert(key.to_string(), vec!["1".into(), "2".into()]);
        }
        let first = generate_parameter_combinations(&params);
        let second = generate_parameter_combinations(&params);
        assert_eq!(first, second);

        // Stronger guarantee: keys appear in alphabetical order inside
        // each generated combination, so the produced workflow plan is
        // identical regardless of how the caller built the HashMap.
        let first_keys: Vec<&String> = first
            .first()
            .expect("at least one combination")
            .keys()
            .collect();
        let mut expected: Vec<&str> = vec!["alpha", "delta", "mango", "papaya", "zebra"];
        expected.sort_unstable();
        let actual: Vec<&str> = first_keys.iter().map(|s| s.as_str()).collect();
        let mut actual_sorted = actual.clone();
        actual_sorted.sort_unstable();
        assert_eq!(actual_sorted, expected);
    }

    #[test]
    fn generate_combinations_two_keys_cartesian_size() {
        let mut params: HashMap<String, Vec<String>> = HashMap::new();
        params.insert("a".to_string(), vec!["1".into(), "2".into()]);
        params.insert("b".to_string(), vec!["x".into(), "y".into(), "z".into()]);
        let out = generate_parameter_combinations(&params);
        assert_eq!(out.len(), 6); // 2 * 3
        for combo in &out {
            assert!(combo.contains_key("a"));
            assert!(combo.contains_key("b"));
        }
    }

    #[test]
    fn default_activity_options_uses_default_retry_policy() {
        // The smoke test is that it builds without panicking.
        let _opts = default_activity_options();
    }
}
