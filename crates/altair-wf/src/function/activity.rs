//! `FunctionActivities` — the stateful Temporal activity that dispatches
//! to registered handlers by name.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::SystemTime;

use altair_temporal::temporalio_common;
#[allow(unused_imports)]
use altair_temporal::temporalio_macros::{activities, activity};
use altair_temporal::temporalio_sdk::activities::{ActivityContext, ActivityError};
use futures::FutureExt as _;

use crate::function::payload::{FunctionExecutionInput, FunctionExecutionOutput};
use crate::function::registry::Registry;
use crate::traits::TaskInput;

/// Stateful activity instance the worker registers via
/// `WorkerBuilder::register_activities`. The instance holds the
/// shared [`Registry`] so multiple activity executions on the same
/// worker dispatch into the same handlers.
pub struct FunctionActivities {
    /// The registry of handlers the activity will dispatch into.
    pub registry: Registry,
}

impl FunctionActivities {
    /// Build a fresh `FunctionActivities` wrapping `registry`.
    #[must_use]
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }
}

#[activities]
impl FunctionActivities {
    /// Dispatch the handler named in `input.name`. Captures panics
    /// inside the handler so a single broken handler doesn't tear the
    /// worker down — the panic is reported via
    /// `FunctionExecutionOutput { success: false, error: ... }`.
    ///
    /// Returns `Err(ActivityError)` only for **infrastructure**
    /// failures (validation, registry lookup miss) that should trigger
    /// Temporal's retry policy. Handler-reported errors and handler
    /// panics are returned as a successful activity completion with
    /// `success = false`, matching the Go original's semantics so the
    /// workflow patterns' `TaskOutput::is_success` plays nicely.
    #[activity]
    pub async fn execute_function(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: FunctionExecutionInput,
    ) -> std::result::Result<FunctionExecutionOutput, ActivityError> {
        let started = std::time::Instant::now();
        let started_at_millis = unix_millis();

        // Infrastructure error — validation.
        if let Err(e) = input.validate() {
            return Err(ActivityError::application(
                temporalio_common::error::ApplicationFailure::builder(anyhow::anyhow!("{e}"))
                    .type_name("InvalidFunctionInput".to_string())
                    .non_retryable(true)
                    .build(),
            ));
        }

        // Infrastructure error — registry miss.
        let handler = match self.registry.get(&input.name) {
            Ok(h) => h,
            Err(e) => {
                return Err(ActivityError::application(
                    temporalio_common::error::ApplicationFailure::builder(anyhow::anyhow!("{e}"))
                        .type_name("FunctionNotFound".to_string())
                        .non_retryable(true)
                        .build(),
                ));
            }
        };

        let fn_input = input.to_function_input();
        // Catch handler panics so a single broken handler can't bring
        // the activity worker down. AssertUnwindSafe is sound here
        // because we only consume `fn_input` and read `input.name`
        // afterwards.
        let result = AssertUnwindSafe(handler(fn_input)).catch_unwind().await;

        let finished = started.elapsed();
        let finished_at_millis = unix_millis();

        let out = match result {
            Ok(Ok(output)) => FunctionExecutionOutput {
                name: input.name.clone(),
                success: true,
                error: String::new(),
                result: output.result,
                data: output.data,
                duration: finished,
                started_at_millis,
                finished_at_millis,
            },
            Ok(Err(e)) => FunctionExecutionOutput {
                name: input.name.clone(),
                success: false,
                error: format!("{e}"),
                result: std::collections::HashMap::new(),
                data: Vec::new(),
                duration: finished,
                started_at_millis,
                finished_at_millis,
            },
            Err(panic) => FunctionExecutionOutput {
                name: input.name.clone(),
                success: false,
                error: render_panic(&*panic),
                result: std::collections::HashMap::new(),
                data: Vec::new(),
                duration: finished,
                started_at_millis,
                finished_at_millis,
            },
        };

        Ok(out)
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn render_panic(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        format!("handler panicked: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("handler panicked: {s}")
    } else {
        "handler panicked: <non-string payload>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::payload::{FunctionInput, FunctionOutput};

    fn make_registry() -> Registry {
        let mut reg = Registry::new();
        reg.register("upper", |i: FunctionInput| async move {
            let v = i.args.get("text").cloned().unwrap_or_default();
            Ok::<_, std::io::Error>(FunctionOutput::with_result([(
                "out".to_string(),
                v.to_uppercase(),
            )]))
        })
        .unwrap();
        reg.register("explode", |_| async move {
            Err::<FunctionOutput, _>(std::io::Error::other("kaboom"))
        })
        .unwrap();
        reg.register("panic", |_| async move {
            panic!("intentional");
            #[allow(unreachable_code)]
            Ok::<FunctionOutput, std::io::Error>(FunctionOutput::default())
        })
        .unwrap();
        reg
    }

    #[test]
    fn render_panic_string_payload() {
        let s = "broken".to_string();
        let payload: Box<dyn std::any::Any + Send> = Box::new(s);
        assert_eq!(render_panic(&*payload), "handler panicked: broken");
    }

    #[test]
    fn render_panic_static_str_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("oops");
        assert_eq!(render_panic(&*payload), "handler panicked: oops");
    }

    #[test]
    fn render_panic_unknown_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(42i32);
        assert_eq!(
            render_panic(&*payload),
            "handler panicked: <non-string payload>"
        );
    }

    // The macro-generated activity surface is exercised by integration
    // tests against a real Temporal container — those run the actual
    // `execute_function` body. The unit tests here cover the helpers
    // and registry; the `make_registry` builder above ensures the
    // shapes line up.
    #[allow(dead_code)]
    fn _assert_registry_builds() {
        let _ = make_registry();
    }
}
