// The RwLock-poison panics in the Registry are
// theoretically reachable but not worth a `# Panics` block on every
// method (the lock would only poison if a previous handler panicked
// while holding it, in which case the whole worker is already in a
// bad state).
#![allow(clippy::missing_panics_doc)]

//! Named-handler dispatch on top of the workflow patterns.
//!
//! The `function` feature ports `go-wf/function/`: a thread-safe
//! [`Registry`] of `String -> Handler` plus a single Temporal activity
//! that looks the handler up by name and runs it. Combined with the
//! workflow patterns from the crate root, you can express "run these
//! 5 jobs as a pipeline" or "fan out 100 named jobs in parallel"
//! without declaring a typed activity per job.
//!
//! ```no_run
//! # #[cfg(feature = "function")] {
//! use altair_wf::function::{FunctionInput, FunctionOutput, Registry};
//!
//! # async fn ex() -> anyhow::Result<()> {
//! let mut reg = Registry::new();
//! reg.register("greet", |input: FunctionInput| async move {
//!     let who = input.args.get("name").cloned().unwrap_or_default();
//!     Ok::<_, std::io::Error>(FunctionOutput::with_result([
//!         ("msg".to_string(), format!("hello {who}"))
//!     ]))
//! })?;
//!
//! let out = reg.dispatch("greet", FunctionInput::new().with_args([("name", "world")])).await?;
//! assert_eq!(out.result.get("msg").unwrap(), "hello world");
//! # Ok(()) }
//! # }
//! ```
//!
//! The Go original exposes `FunctionExecutionInput` / `FunctionExecutionOutput`
//! as the activity payload — these are kept as separate, validated
//! types implementing [`crate::TaskInput`] / [`crate::TaskOutput`] so
//! the patterns can drive them directly.

mod activity;
mod payload;
mod registry;

pub use activity::FunctionActivities;
pub use payload::{
    DEFAULT_FUNCTION_ACTIVITY_NAME, FunctionExecutionInput, FunctionExecutionOutput, FunctionInput,
    FunctionOutput,
};
pub use registry::{HandlerError, Registry};
