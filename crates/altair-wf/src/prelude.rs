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

pub use crate::{
    DAGInput, DAGNode, DAGOutput, Error, FailureStrategy, LoopInput, LoopOutput, NodeResult,
    ParallelInput, ParallelOutput, ParameterizedLoopInput, PipelineInput, PipelineOutput, Result,
    Substitutor, TaskInput, TaskOutput, default_activity_options, default_retry_policy, execute,
    execute_with_timeout, generate_parameter_combinations, parallel, parameterized_loop, pipeline,
    run_dag, run_loop, substitute_template,
};
