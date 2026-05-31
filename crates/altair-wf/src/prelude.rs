//! Convenience re-exports — one `use altair_wf::prelude::*;` is enough
//! to write workflows using the patterns shipped here.
//!
//! ```
//! use altair_wf::prelude::*;
//!
//! # #[derive(Clone, serde::Serialize, serde::Deserialize)]
//! # struct MyTask;
//! # impl TaskInput for MyTask {}
//! # #[derive(serde::Serialize, serde::Deserialize)]
//! # struct MyResult { ok: bool }
//! # impl TaskOutput for MyResult { fn is_success(&self) -> bool { self.ok } }
//! # async fn ex() -> altair_wf::Result<()> {
//! let input = PipelineInput { tasks: vec![MyTask], stop_on_error: false, cleanup: false };
//! let _out: PipelineOutput<MyResult> = pipeline(input, |t| async move {
//!     // call ctx.start_activity(...).await here in real code
//!     Ok(MyResult { ok: true })
//! }).await?;
//! # Ok(()) }
//! ```
//!
//! # Footgun: `Result` shadowing inside the Temporal SDK macros
//!
//! This prelude re-exports `Result<T>` — the crate's one-arg alias for
//! `std::result::Result<T, altair_wf::Error>`. **Do not** glob-import
//! the prelude in a module that contains an `#[activity]` or
//! `#[workflow_methods]` impl block from the Temporal SDK macros. Those
//! macros expand to code using `Result<T, ActivityError>` (two
//! generics) and the 1-arg alias swallows the second parameter, leaving
//! you with a confusing `type alias takes 1 generic argument but 2 were
//! supplied` error.
//!
//! In workflow / activity modules, prefer explicit imports:
//!
//! ```ignore
//! // GOOD inside a #[workflow_methods] impl
//! use altair_wf::{PipelineInput, PipelineOutput, pipeline};
//! use altair_wf::{TaskInput, TaskOutput};
//!
//! // BAD — would shadow std::result::Result
//! // use altair_wf::prelude::*;
//! ```
//!
//! # `function` feature re-exports
//!
//! When the `function` feature is enabled, this prelude also re-exports
//! the named-handler types ([`Registry`], [`FunctionActivities`],
//! [`FunctionInput`] / [`FunctionOutput`], [`FunctionExecutionInput`] /
//! [`FunctionExecutionOutput`]) so a single glob import gives you both
//! the patterns and the registry surface.
//!
//! [`Registry`]: crate::function::Registry
//! [`FunctionActivities`]: crate::function::FunctionActivities
//! [`FunctionInput`]: crate::function::FunctionInput
//! [`FunctionOutput`]: crate::function::FunctionOutput
//! [`FunctionExecutionInput`]: crate::function::FunctionExecutionInput
//! [`FunctionExecutionOutput`]: crate::function::FunctionExecutionOutput

pub use crate::{
    DAGInput, DAGNode, DAGOutput, Error, FailureStrategy, LoopInput, LoopOutput, NodeResult,
    ParallelInput, ParallelOutput, ParameterizedLoopInput, PipelineInput, PipelineOutput, Result,
    Substitutor, TaskInput, TaskOutput, default_activity_options, default_retry_policy, execute,
    execute_with_timeout, generate_parameter_combinations, parallel, parameterized_loop, pipeline,
    run_dag, run_loop, substitute_template, substitutor_from_fn,
};

// Mirror lib.rs's altair-temporal facade re-exports so a single
// `use altair_wf::prelude::*;` (in a module that does NOT also host
// the SDK macros — see the footgun warning above) gives consumers
// everything they need to define and run a workflow.
pub use crate::{
    ActivityContext, ActivityError, ActivityOptions, Client, Config, RetryPolicy, Worker,
    WorkerBuilder, WorkflowContext, WorkflowResult,
};

// `function` module re-exports so feature-enabled consumers can opt
// in with a single glob import. `HandlerError` is intentionally
// excluded — users supply concrete errors satisfying `std::error::Error`,
// they don't construct or pattern-match the boxed alias.
#[cfg(feature = "function")]
pub use crate::function::{
    FunctionActivities, FunctionExecutionInput, FunctionExecutionOutput, FunctionInput,
    FunctionOutput, Registry,
};
