//! Input + output payload types for each pattern.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::helpers::FailureStrategy;
use crate::traits::TaskInput;

/// A user-supplied closure that turns the template input + loop position
/// into a concrete per-iteration input.
///
/// Wrapped in `Arc<dyn Fn ...>` so it can be cloned cheaply and called
/// from multiple parallel-loop futures. Use [`Arc::new`] to construct.
pub type Substitutor<I> =
    Arc<dyn Fn(&I, &str, usize, &std::collections::HashMap<String, String>) -> I + Send + Sync>;

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// Input to the pipeline (sequential) pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineInput<I> {
    /// The ordered list of activity inputs. Must be non-empty.
    pub tasks: Vec<I>,
    /// Abort the pipeline on the first failing step. Default `false`.
    #[serde(default)]
    pub stop_on_error: bool,
    /// Reserved for future cleanup-on-failure semantics; not currently
    /// acted on by the pattern.
    #[serde(default)]
    pub cleanup: bool,
}

impl<I: TaskInput> PipelineInput<I> {
    /// Validate that the task list is non-empty and that every entry's
    /// own `validate()` succeeds.
    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() {
            return Err(Error::InvalidInput(
                "pipeline tasks must be non-empty".into(),
            ));
        }
        for (i, task) in self.tasks.iter().enumerate() {
            task.validate()
                .map_err(|e| Error::InvalidInput(format!("task[{i}]: {e}")))?;
        }
        Ok(())
    }
}

/// Aggregated result of a pipeline run.
///
/// **No wall-clock duration field.** Patterns are invoked from inside a
/// Temporal `#[run]` workflow body where `Instant::now()` is replay-
/// non-deterministic — measuring it here would corrupt history. Use
/// Temporal's built-in workflow execution metrics, or measure inside
/// the activities themselves (allowed; activities have non-deterministic
/// contexts).
///
/// # Failure addressing
///
/// `results` is shorter than `tasks` when an activity error happens
/// under `stop_on_error = false` (the error path has no `O` value to
/// push). When that happens, indices in `results` no longer correspond
/// to input positions. Use [`failed_indices`](Self::failed_indices) +
/// [`failure_reasons`](Self::failure_reasons) to recover *which*
/// input task failed and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutput<O> {
    /// Per-step results in execution order. May be shorter than
    /// `tasks.len()` when `stop_on_error` is true or when any task's
    /// dispatch closure returned `Err` (activity error) under
    /// `stop_on_error = false`.
    pub results: Vec<O>,
    /// Count of steps whose `is_success()` returned `true`.
    pub total_success: usize,
    /// Count of failing steps.
    pub total_failed: usize,
    /// Input positions (0-based, into `PipelineInput::tasks`) that
    /// failed — same length and order as [`failure_reasons`]. Covers
    /// both business-logic failures (`is_success()` returned false)
    /// and activity errors (the dispatch closure returned `Err`).
    ///
    /// Omitted from the wire format when empty (so the JSON is small
    /// and a downstream `null` vs `[]` distinction never surfaces);
    /// deserialises as an empty vec when the field is absent.
    ///
    /// [`failure_reasons`]: Self::failure_reasons
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failed_indices: Vec<usize>,
    /// One error message per `failed_indices` entry. For
    /// business-logic failures this is `TaskOutput::error()`; for
    /// activity errors this is the closure's `Err` rendered via
    /// `Display`.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failure_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parallel
// ---------------------------------------------------------------------------

/// Input to the parallel pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelInput<I> {
    /// Activity inputs.
    pub tasks: Vec<I>,
    /// What to do when a task fails. Default [`FailureStrategy::Continue`].
    #[serde(default, with = "failure_strategy_serde")]
    pub failure_strategy: FailureStrategy,
    /// Maximum number of in-flight task futures at any moment.
    ///
    /// `0` (the default) means "no cap" — every task future is built
    /// and polled up front, the historical behaviour. Set a positive
    /// integer to bound memory and concurrency: at most `max_in_flight`
    /// futures are alive simultaneously. This is **independent** of the
    /// worker's `max_concurrent_activities` slot count — the worker cap
    /// throttles activity dispatch, this cap throttles **how many
    /// dispatch futures the pattern itself holds**, which matters for
    /// payload memory and pre-dispatch validation cost on very wide
    /// fan-outs (think 10K+ tasks).
    #[serde(default)]
    pub max_in_flight: usize,
}

impl<I: TaskInput> ParallelInput<I> {
    /// Validate task list non-empty + per-task validate.
    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() {
            return Err(Error::InvalidInput(
                "parallel tasks must be non-empty".into(),
            ));
        }
        for (i, task) in self.tasks.iter().enumerate() {
            task.validate()
                .map_err(|e| Error::InvalidInput(format!("task[{i}]: {e}")))?;
        }
        Ok(())
    }
}

/// Aggregated result of a parallel run.
///
/// See [`PipelineOutput`] for why there's no wall-clock duration field
/// and for the failure-addressing contract on the `failed_indices` /
/// `failure_reasons` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelOutput<O> {
    /// Per-task results. Ordering matches `tasks` ordering, not
    /// completion order. Shorter than `tasks.len()` when any task's
    /// dispatch closure returned `Err` (activity error) under
    /// `FailureStrategy::Continue` — see `failed_indices` to recover
    /// the missing positions.
    pub results: Vec<O>,
    /// Count of tasks whose `is_success()` returned `true`.
    pub total_success: usize,
    /// Count of failing tasks.
    pub total_failed: usize,
    /// Input positions (0-based, into `ParallelInput::tasks`) that
    /// failed. Omitted from the wire format when empty.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failed_indices: Vec<usize>,
    /// One error message per `failed_indices` entry. Omitted from the
    /// wire format when empty.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failure_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Loop
// ---------------------------------------------------------------------------

/// Input to the loop pattern — one activity invocation per item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopInput<I> {
    /// Items to iterate. Must be non-empty.
    pub items: Vec<String>,
    /// Template input — passed to the substitutor on every iteration to
    /// produce the per-iteration concrete input.
    pub template: I,
    /// Whether to run iterations in parallel.
    #[serde(default)]
    pub parallel: bool,
    /// Failure strategy.
    #[serde(default, with = "failure_strategy_serde")]
    pub failure_strategy: FailureStrategy,
    /// In-flight cap when [`parallel`] is true. `0` (default) means no
    /// cap. See [`ParallelInput::max_in_flight`] for the operational
    /// rationale.
    ///
    /// [`parallel`]: Self::parallel
    /// [`ParallelInput::max_in_flight`]: crate::ParallelInput::max_in_flight
    #[serde(default)]
    pub max_in_flight: usize,
}

impl<I: TaskInput> LoopInput<I> {
    /// Validate items non-empty + template's validate.
    pub fn validate(&self) -> Result<()> {
        if self.items.is_empty() {
            return Err(Error::InvalidInput("loop items must be non-empty".into()));
        }
        self.template
            .validate()
            .map_err(|e| Error::InvalidInput(format!("template: {e}")))
    }
}

/// Input to the parameterized-loop pattern — runs once per cartesian
/// combination of parameter values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterizedLoopInput<I> {
    /// Each key maps to a non-empty list of values. The pattern runs
    /// once per cartesian product.
    pub parameters: std::collections::HashMap<String, Vec<String>>,
    /// Template input.
    pub template: I,
    /// Whether to run combinations in parallel.
    #[serde(default)]
    pub parallel: bool,
    /// Failure strategy.
    #[serde(default, with = "failure_strategy_serde")]
    pub failure_strategy: FailureStrategy,
    /// In-flight cap when [`parallel`] is true. `0` (default) means no
    /// cap. See [`ParallelInput::max_in_flight`] for the operational
    /// rationale.
    ///
    /// [`parallel`]: Self::parallel
    /// [`ParallelInput::max_in_flight`]: crate::ParallelInput::max_in_flight
    #[serde(default)]
    pub max_in_flight: usize,
}

impl<I: TaskInput> ParameterizedLoopInput<I> {
    /// Validate parameters non-empty, no empty value lists, template.
    pub fn validate(&self) -> Result<()> {
        if self.parameters.is_empty() {
            return Err(Error::InvalidInput(
                "parameterized loop parameters must be non-empty".into(),
            ));
        }
        for (k, vs) in &self.parameters {
            if vs.is_empty() {
                return Err(Error::InvalidInput(format!(
                    "parameter '{k}' value list must be non-empty"
                )));
            }
        }
        self.template
            .validate()
            .map_err(|e| Error::InvalidInput(format!("template: {e}")))
    }
}

/// Aggregated result of a loop or parameterised-loop run.
///
/// See [`PipelineOutput`] for why there's no wall-clock duration field
/// and for the failure-addressing contract on the `failed_indices` /
/// `failure_reasons` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopOutput<O> {
    /// Per-iteration results in iteration order.
    pub results: Vec<O>,
    /// Successes.
    pub total_success: usize,
    /// Failures.
    pub total_failed: usize,
    /// Number of iterations attempted (`items.len()` or the size of the
    /// cartesian product).
    pub item_count: usize,
    /// Iteration indices (0-based) that failed. Omitted from the
    /// wire format when empty.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failed_indices: Vec<usize>,
    /// One error message per `failed_indices` entry. Omitted from the
    /// wire format when empty.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub failure_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Serde shims
// ---------------------------------------------------------------------------

/// `serde` adapter so the `FailureStrategy` enum (de)serialises as the
/// same strings the Go counterpart uses: `"fail_fast"`, `"continue"`,
/// or `""` (treated as `Continue`).
mod failure_strategy_serde {
    use super::FailureStrategy;
    use serde::{Deserialize, Deserializer, Serializer};

    // serde's `serialize` callsite passes a reference; intentional.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(s: &FailureStrategy, ser: S) -> Result<S::Ok, S::Error> {
        let v = match s {
            FailureStrategy::Continue => "continue",
            FailureStrategy::FailFast => "fail_fast",
        };
        ser.serialize_str(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<FailureStrategy, D::Error> {
        let s = String::deserialize(de)?;
        Ok(match s.as_str() {
            "" | "continue" => FailureStrategy::Continue,
            "fail_fast" => FailureStrategy::FailFast,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown failure_strategy '{other}', expected 'continue' or 'fail_fast'"
                )));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Step {
        ok: bool,
    }
    impl TaskInput for Step {
        fn validate(&self) -> Result<()> {
            if self.ok {
                Ok(())
            } else {
                Err(Error::InvalidInput("step not ok".into()))
            }
        }
    }

    #[test]
    fn pipeline_input_rejects_empty() {
        let p: PipelineInput<Step> = PipelineInput {
            tasks: vec![],
            stop_on_error: false,
            cleanup: false,
        };
        assert!(matches!(p.validate(), Err(Error::InvalidInput(_))));
    }

    #[test]
    fn pipeline_input_propagates_per_task_validate_failure() {
        let p = PipelineInput {
            tasks: vec![Step { ok: true }, Step { ok: false }],
            stop_on_error: false,
            cleanup: false,
        };
        match p.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("task[1]")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn parameterized_loop_rejects_empty_value_list() {
        use std::collections::HashMap;
        let mut params: HashMap<String, Vec<String>> = HashMap::new();
        params.insert("k".into(), vec![]);
        let p = ParameterizedLoopInput {
            parameters: params,
            template: Step { ok: true },
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
            max_in_flight: 0,
        };
        match p.validate() {
            Err(Error::InvalidInput(msg)) => assert!(msg.contains("'k'")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn failure_strategy_serde_round_trip() {
        let p = ParallelInput::<Step> {
            tasks: vec![Step { ok: true }],
            failure_strategy: FailureStrategy::FailFast,
            max_in_flight: 0,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("fail_fast"));
        let r: ParallelInput<Step> = serde_json::from_str(&s).unwrap();
        assert_eq!(r.failure_strategy, FailureStrategy::FailFast);
    }

    #[test]
    fn failure_strategy_default_is_continue() {
        let raw = r#"{"tasks":[{"ok":true}]}"#;
        let r: ParallelInput<Step> = serde_json::from_str(raw).unwrap();
        assert_eq!(r.failure_strategy, FailureStrategy::Continue);
    }

    #[test]
    fn failure_strategy_empty_string_is_continue() {
        let raw = r#"{"tasks":[{"ok":true}],"failure_strategy":""}"#;
        let r: ParallelInput<Step> = serde_json::from_str(raw).unwrap();
        assert_eq!(r.failure_strategy, FailureStrategy::Continue);
    }
}
