//! `FunctionActivities` — the stateful Temporal activity that dispatches
//! to registered handlers by name.

// The SDK's #[activities] macro expands to consts and trait impls
// that don't carry doc comments — relax the missing docs check here.
#![allow(missing_docs)]

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
        ctx: ActivityContext,
        input: FunctionExecutionInput,
    ) -> std::result::Result<FunctionExecutionOutput, ActivityError> {
        tracing::info!(handler = %input.name, "function.execute: dispatching");
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

        // Consume `input` into its name and a handler payload so we
        // don't clone the `args`/`data`/`env`/`work_dir` maps and vecs
        // on the hot path.
        let name = input.name.clone();
        let fn_input = input.into_function_input();

        // If the activity options configure a heartbeat timeout,
        // spawn a ticker that records a heartbeat at half the timeout
        // interval. Without this, a long-running handler under a
        // `heartbeat_timeout` setting would be killed even though it
        // was making progress. The ticker is dropped (and the loop
        // ends) the moment the handler future completes.
        let heartbeat_interval = ctx.info().heartbeat_timeout.map(|d| d / 2);
        let heartbeat_fut = async {
            if let Some(interval) = heartbeat_interval
                && !interval.is_zero()
            {
                let mut ticker = tokio::time::interval(interval);
                // Skip the immediate first tick; first record fires
                // at `interval` elapsed.
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    ctx.record_heartbeat(Vec::new());
                }
            }
            // No heartbeat configured — sleep forever; the select will
            // drop us when the handler finishes.
            std::future::pending::<()>().await;
        };

        // Catch handler panics so a single broken handler can't bring
        // the activity worker down. `AssertUnwindSafe` is sound here:
        // the inner future owns `fn_input` (moved in) and the wrapper
        // stored in the registry holds an `Arc<dyn Fn>` with no
        // interior mutability, so a mid-poll panic drops the future's
        // state cleanly without leaving any shared state inconsistent.
        let handler_fut = AssertUnwindSafe(handler(fn_input)).catch_unwind();
        tokio::pin!(heartbeat_fut);
        let result = tokio::select! {
            biased;
            r = handler_fut => r,
            // Heartbeat loop never returns; this arm exists only to
            // keep the ticker running concurrently. select! drops the
            // losing branch when the handler completes.
            () = &mut heartbeat_fut => unreachable!("heartbeat loop should never resolve"),
        };

        let finished = started.elapsed();
        let finished_at_millis = unix_millis();
        // u64 nanos covers ~584 years; saturation only matters for
        // pathological/test handlers.
        let elapsed_nanos = u64::try_from(finished.as_nanos()).unwrap_or(u64::MAX);

        let out = match result {
            Ok(Ok(output)) => FunctionExecutionOutput {
                name,
                success: true,
                error: String::new(),
                result: output.result,
                data: output.data,
                duration: finished,
                elapsed_nanos,
                started_at_millis,
                finished_at_millis,
            },
            Ok(Err(e)) => FunctionExecutionOutput {
                name,
                success: false,
                // A user `Display` impl that itself panics could escape
                // the future's `catch_unwind` above (the call site lives
                // outside the polled future). Catch it explicitly and
                // fall back to a static message.
                error: render_handler_error(&*e),
                result: std::collections::HashMap::new(),
                data: Vec::new(),
                duration: finished,
                elapsed_nanos,
                started_at_millis,
                finished_at_millis,
            },
            Err(panic) => FunctionExecutionOutput {
                name,
                success: false,
                error: render_panic(&*panic),
                result: std::collections::HashMap::new(),
                data: Vec::new(),
                duration: finished,
                elapsed_nanos,
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

/// Render a user error to a string, with a defence-in-depth
/// `catch_unwind` around the `Display` call. A user `Display` impl
/// that itself panics would otherwise propagate out of the activity
/// body (it's evaluated outside the future's `catch_unwind`).
fn render_handler_error(e: &(dyn std::error::Error + Send + Sync)) -> String {
    std::panic::catch_unwind(AssertUnwindSafe(|| format!("{e}")))
        .unwrap_or_else(|_| "<handler error: Display impl panicked>".to_string())
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
