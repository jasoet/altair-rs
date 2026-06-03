//! Payload types: handler input/output + Temporal activity
//! input/output.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::traits::{TaskInput, TaskOutput};

/// Default activity name (`"ExecuteFunctionActivity"`).
///
/// **Not used by the Rust activity** — the Temporal SDK's
/// `#[activity]` macro derives the activity name from the method
/// (`execute_function`). This constant is exposed so cross-language
/// workflows (Go workers talking to Rust workers or vice versa) can
/// agree on a single wire name when needed; explicitly pass it via
/// the SDK's `name` attribute to override the default.
pub const DEFAULT_FUNCTION_ACTIVITY_NAME: &str = "ExecuteFunctionActivity";

// ---------------------------------------------------------------------------
// Handler-side payloads
// ---------------------------------------------------------------------------

/// Input passed to a registered handler. All fields are optional —
/// callers wire only what they need.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionInput {
    /// Named scalar arguments.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, String>,
    /// Bulk binary payload (e.g. uploaded file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
    /// Environment-variable-style key/values.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// Working directory hint.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub work_dir: String,
}

impl FunctionInput {
    /// Build an empty `FunctionInput`. Chain `with_*` to populate.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace `args` and return `self` for chaining.
    ///
    /// Mirrors [`FunctionExecutionInput::with_args`] so the two payload
    /// types share the same builder idiom.
    #[must_use]
    pub fn with_args<I, K, V>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.args = args
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self
    }

    /// Replace `data` and return `self` for chaining.
    #[must_use]
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = data.into();
        self
    }

    /// Replace `env` and return `self` for chaining.
    #[must_use]
    pub fn with_env<I, K, V>(mut self, env: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        self
    }

    /// Replace `work_dir` and return `self` for chaining.
    #[must_use]
    pub fn with_work_dir(mut self, work_dir: impl Into<String>) -> Self {
        self.work_dir = work_dir.into();
        self
    }
}

/// Output returned by a registered handler.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionOutput {
    /// Named scalar results.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub result: HashMap<String, String>,
    /// Bulk binary payload (e.g. generated file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
}

impl FunctionOutput {
    /// Convenience: build a `FunctionOutput` from an iterator of
    /// `(name, value)` result pairs.
    pub fn with_result<I, K, V>(result: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            result: result
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Activity-side payloads (impl TaskInput / TaskOutput so patterns can drive them)
// ---------------------------------------------------------------------------

/// Activity payload — names the handler to dispatch and carries all the
/// fields the handler will see, plus a few activity-level options.
///
/// Implements [`crate::TaskInput`] so the workflow patterns
/// (`pipeline`, `parallel`, `run_loop`, …) can drive these directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionExecutionInput {
    /// Name of the registered handler to invoke.
    pub name: String,
    /// Forwarded as `FunctionInput::args`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, String>,
    /// Forwarded as `FunctionInput::data`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
    /// Forwarded as `FunctionInput::env`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// Forwarded as `FunctionInput::work_dir`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub work_dir: String,
    /// Reserved for a future per-invocation timeout hint. Not enforced
    /// by the activity — set the timeout via `ActivityOptions` instead.
    #[serde(default, with = "duration_millis_compat")]
    pub timeout: Duration,
    /// Free-form metadata for traces / dashboards. Not consumed by the
    /// activity.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,
}

impl FunctionExecutionInput {
    /// Helper: build an input from a handler name + arg map.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Replace `args` and return `self` for chaining.
    #[must_use]
    pub fn with_args<I, K, V>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.args = args
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self
    }

    /// Replace `data` and return `self` for chaining.
    #[must_use]
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = data.into();
        self
    }

    /// Replace `env` and return `self` for chaining.
    #[must_use]
    pub fn with_env<I, K, V>(mut self, env: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        self
    }

    /// Replace `work_dir` and return `self` for chaining.
    #[must_use]
    pub fn with_work_dir(mut self, work_dir: impl Into<String>) -> Self {
        self.work_dir = work_dir.into();
        self
    }

    /// Replace `timeout` and return `self` for chaining. The timeout is
    /// metadata only — the activity itself does not enforce it; configure
    /// the real cutoff via `ActivityOptions::start_to_close_timeout`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace `labels` and return `self` for chaining.
    #[must_use]
    pub fn with_labels<I, K, V>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.labels = labels
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self
    }

    /// Project to a [`FunctionInput`] for handler dispatch, cloning
    /// the forwarded fields. Use [`Self::into_function_input`] to
    /// avoid the clones when you no longer need the execution input.
    #[must_use]
    pub fn to_function_input(&self) -> FunctionInput {
        FunctionInput {
            args: self.args.clone(),
            data: self.data.clone(),
            env: self.env.clone(),
            work_dir: self.work_dir.clone(),
        }
    }

    /// Consume `self` into a [`FunctionInput`] for handler dispatch.
    /// Avoids the field clones of [`Self::to_function_input`].
    #[must_use]
    pub fn into_function_input(self) -> FunctionInput {
        FunctionInput {
            args: self.args,
            data: self.data,
            env: self.env,
            work_dir: self.work_dir,
        }
    }
}

impl TaskInput for FunctionExecutionInput {
    /// Only validates `name` — checks that it matches the Go original's
    /// `^[a-zA-Z][a-zA-Z0-9_-]*$` pattern and is ≤255 bytes long. All
    /// other fields (`args`, `data`, `env`, `work_dir`, `timeout`,
    /// `labels`) are passed through to the handler unchecked; bound
    /// them at the handler boundary if needed.
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::InvalidInput("function name is required".into()));
        }
        // Match the Go original's regex `^[a-zA-Z][a-zA-Z0-9_-]*$` so
        // names are safe to embed in event-history / metrics / logs.
        let mut chars = self.name.chars();
        let first = chars.next().unwrap_or('_');
        if !first.is_ascii_alphabetic() {
            return Err(Error::InvalidInput(format!(
                "function name {:?} must start with an ASCII letter",
                self.name
            )));
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(Error::InvalidInput(format!(
                    "function name {:?} may only contain ASCII letters, digits, '_' or '-'",
                    self.name
                )));
            }
        }
        if self.name.len() > 255 {
            return Err(Error::InvalidInput(
                "function name must be <= 255 characters".into(),
            ));
        }
        Ok(())
    }
}

/// Activity output — captures success / error, the handler's result,
/// and timing for observability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionExecutionOutput {
    /// Echo of the dispatched handler name.
    pub name: String,
    /// `true` when the handler completed without erroring.
    pub success: bool,
    /// Handler error message when `success == false`. Empty otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    /// Handler `result` map.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub result: HashMap<String, String>,
    /// Handler `data` payload.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
    /// Wall-clock duration of the handler call, **rounded down to
    /// whole milliseconds** on the wire. Reports `0` for sub-ms
    /// handlers — use [`elapsed_nanos`](Self::elapsed_nanos) for
    /// fine-grained observability.
    ///
    /// **Observational only.** Branching on this field from inside a
    /// workflow body breaks Temporal replay determinism — the activity
    /// is non-deterministic, so the measured duration depends on
    /// machine + load and changes between executions. Read it for
    /// metrics emitted at the activity boundary or for non-workflow
    /// observability; do not let it influence workflow control flow.
    #[serde(with = "duration_millis_compat")]
    pub duration: Duration,
    /// Wall-clock handler duration in **nanoseconds** — distinct from
    /// `duration` (millis) so sub-millisecond handlers still surface
    /// real timing data. Wire format: bare `u64`. A `u64` of nanos
    /// covers ~584 years, so saturation is not a practical concern.
    ///
    /// Observational only — same workflow-determinism caveat as
    /// [`duration`](Self::duration).
    #[serde(default)]
    pub elapsed_nanos: u64,
    /// Unix-millis when the handler started. Zero on the default
    /// instance.
    ///
    /// Observational only — see [`duration`](Self::duration).
    #[serde(default)]
    pub started_at_millis: u64,
    /// Unix-millis when the handler finished. Zero on the default
    /// instance.
    ///
    /// Observational only — see [`duration`](Self::duration).
    #[serde(default)]
    pub finished_at_millis: u64,
}

impl FunctionExecutionOutput {
    /// Construct a successful output.
    pub fn success(name: impl Into<String>, output: FunctionOutput) -> Self {
        Self {
            name: name.into(),
            success: true,
            result: output.result,
            data: output.data,
            ..Self::default()
        }
    }

    /// Construct a failure output.
    pub fn failure(name: impl Into<String>, err: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            success: false,
            error: err.into(),
            ..Self::default()
        }
    }
}

impl TaskOutput for FunctionExecutionOutput {
    fn is_success(&self) -> bool {
        self.success
    }
    fn error(&self) -> Option<&str> {
        if self.error.is_empty() {
            None
        } else {
            Some(self.error.as_str())
        }
    }
}

/// Serde adapter that encodes [`Duration`] as `u64` millis on the wire.
///
/// Round-trips losslessly for any duration `< u64::MAX` millis (~584M
/// years); larger durations saturate at `u64::MAX` on serialize.
mod duration_millis_compat {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u64(d.as_millis().try_into().unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Duration, D::Error> {
        u64::deserialize(de).map(Duration::from_millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_input_rejects_empty_name() {
        let i = FunctionExecutionInput::new("");
        assert!(matches!(i.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn execution_input_rejects_name_starting_with_digit() {
        let i = FunctionExecutionInput::new("1bad");
        assert!(matches!(i.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn execution_input_rejects_name_with_space() {
        let i = FunctionExecutionInput::new("bad name");
        assert!(matches!(i.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn execution_input_accepts_letters_digits_underscores_hyphens() {
        let i = FunctionExecutionInput::new("Good_Name-1");
        assert!(i.validate().is_ok());
    }

    #[test]
    fn execution_input_rejects_overlong_name() {
        let long = "a".repeat(300);
        let i = FunctionExecutionInput::new(long);
        assert!(matches!(i.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn to_function_input_copies_fields() {
        let mut i = FunctionExecutionInput::new("ok").with_args([("k", "v")]);
        i.work_dir = "/tmp".into();
        let fi = i.to_function_input();
        assert_eq!(fi.args.get("k").map(String::as_str), Some("v"));
        assert_eq!(fi.work_dir, "/tmp");
    }

    #[test]
    fn output_success_and_failure_helpers() {
        let s = FunctionExecutionOutput::success("f", FunctionOutput::with_result([("x", "1")]));
        assert!(s.is_success());
        assert_eq!(s.error(), None);

        let f = FunctionExecutionOutput::failure("g", "boom");
        assert!(!f.is_success());
        assert_eq!(f.error(), Some("boom"));
    }

    #[test]
    fn execution_input_accepts_255_byte_name_and_rejects_256() {
        let ok = FunctionExecutionInput::new("a".repeat(255));
        assert!(ok.validate().is_ok());
        let too_long = FunctionExecutionInput::new("a".repeat(256));
        assert!(matches!(too_long.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn execution_input_round_trips_through_serde() {
        let original = FunctionExecutionInput::new("fn")
            .with_args([("k", "v")])
            .with_data(vec![1u8, 2, 3])
            .with_env([("API_KEY", "x")])
            .with_work_dir("/tmp")
            .with_timeout(Duration::from_millis(1234))
            .with_labels([("trace", "1")]);
        let json = serde_json::to_string(&original).unwrap();
        let back: FunctionExecutionInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "fn");
        assert_eq!(back.args.get("k").map(String::as_str), Some("v"));
        assert_eq!(back.data, vec![1, 2, 3]);
        assert_eq!(back.env.get("API_KEY").map(String::as_str), Some("x"));
        assert_eq!(back.work_dir, "/tmp");
        assert_eq!(back.timeout, Duration::from_millis(1234));
        assert_eq!(back.labels.get("trace").map(String::as_str), Some("1"));
    }

    #[test]
    fn execution_output_round_trips_through_serde_with_millis_duration() {
        let out = FunctionExecutionOutput {
            name: "fn".into(),
            success: true,
            error: String::new(),
            result: HashMap::from([("k".into(), "v".into())]),
            data: vec![9],
            duration: Duration::from_millis(987),
            elapsed_nanos: 987_000_000,
            started_at_millis: 1_000,
            finished_at_millis: 1_987,
        };
        let value = serde_json::to_value(&out).unwrap();
        // Duration is encoded as a bare millis u64 on the wire.
        assert_eq!(
            value.get("duration").and_then(serde_json::Value::as_u64),
            Some(987)
        );
        // `elapsed_nanos` round-trips alongside the millis field for
        // sub-millisecond observability.
        assert_eq!(
            value
                .get("elapsed_nanos")
                .and_then(serde_json::Value::as_u64),
            Some(987_000_000)
        );
        let back: FunctionExecutionOutput = serde_json::from_value(value).unwrap();
        assert_eq!(back.name, "fn");
        assert!(back.success);
        assert_eq!(back.duration, Duration::from_millis(987));
        assert_eq!(back.started_at_millis, 1_000);
        assert_eq!(back.finished_at_millis, 1_987);
        assert_eq!(back.result.get("k").map(String::as_str), Some("v"));
        assert_eq!(back.data, vec![9]);
    }
}
