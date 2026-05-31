//! Pattern implementations: single-task, pipeline, parallel, loop,
//! parameterised loop, DAG.
//!
//! Each pattern is SDK-agnostic — it takes an `execute_one` closure that
//! the caller wires to `WorkflowContext::start_activity`. This keeps the
//! orchestration logic pure (unit-testable without Temporal) and lets
//! the user own all activity dispatch.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use std::time::Instant;

use futures::future::join_all;

use crate::dag::{DAGInput, DAGOutput, NodeResult};
use crate::error::{Error, Result};
use crate::helpers::{FailureStrategy, generate_parameter_combinations};
use crate::traits::{TaskInput, TaskOutput};
use crate::types::{
    LoopInput, LoopOutput, ParallelInput, ParallelOutput, ParameterizedLoopInput, PipelineInput,
    PipelineOutput, Substitutor,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Closures returning futures need a small dance to satisfy the
/// borrow-checker when called in a loop. This is the canonical bound.
async fn dispatch<F, Fut, I, O>(execute_one: &mut F, input: I) -> Result<O>
where
    F: FnMut(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    execute_one(input).await
}

// ---------------------------------------------------------------------------
// Single task
// ---------------------------------------------------------------------------

/// Validate the input and dispatch a single task.
pub async fn execute<F, Fut, I, O>(input: I, mut execute_one: F) -> Result<O>
where
    I: TaskInput,
    O: TaskOutput,
    F: FnMut(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input
        .validate()
        .map_err(|e| Error::InvalidInput(e.to_string()))?;
    execute_one(input).await
}

/// `execute` with a per-call timeout enforced by `tokio::time::timeout`.
/// Returns [`Error::PatternStopped`] on timeout.
pub async fn execute_with_timeout<F, Fut, I, O>(
    input: I,
    timeout: Duration,
    execute_one: F,
) -> Result<O>
where
    I: TaskInput,
    O: TaskOutput,
    F: FnMut(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    match tokio::time::timeout(timeout, execute(input, execute_one)).await {
        Ok(res) => res,
        Err(_) => Err(Error::PatternStopped {
            position: "0".into(),
            reason: format!("execute timed out after {timeout:?}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// Pipeline (sequential)
// ---------------------------------------------------------------------------

/// Run `input.tasks` sequentially. On the first failure, returns
/// [`Error::PatternStopped`] if `stop_on_error` is set, otherwise
/// continues.
pub async fn pipeline<F, Fut, I, O>(
    input: PipelineInput<I>,
    mut execute_one: F,
) -> Result<PipelineOutput<O>>
where
    I: TaskInput,
    O: TaskOutput,
    F: FnMut(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input.validate()?;
    let start = Instant::now();
    let mut results: Vec<O> = Vec::with_capacity(input.tasks.len());
    let mut total_success = 0usize;
    let mut total_failed = 0usize;

    for (i, task) in input.tasks.into_iter().enumerate() {
        match dispatch(&mut execute_one, task).await {
            Ok(out) => {
                if out.is_success() {
                    total_success += 1;
                } else {
                    total_failed += 1;
                    if input.stop_on_error {
                        let reason = out.error().unwrap_or("task reported failure").to_string();
                        results.push(out);
                        return Err(Error::PatternStopped {
                            position: i.to_string(),
                            reason,
                        });
                    }
                }
                results.push(out);
            }
            Err(e) => {
                total_failed += 1;
                if input.stop_on_error {
                    return Err(Error::PatternStopped {
                        position: i.to_string(),
                        reason: e.to_string(),
                    });
                }
                // Non-stop: drop the output (we have no value to push) and continue.
            }
        }
    }

    Ok(PipelineOutput {
        results,
        total_success,
        total_failed,
        total_duration: start.elapsed(),
    })
}

// ---------------------------------------------------------------------------
// Parallel
// ---------------------------------------------------------------------------

/// Run every task in `input.tasks` concurrently.
pub async fn parallel<F, Fut, I, O>(
    input: ParallelInput<I>,
    execute_one: F,
) -> Result<ParallelOutput<O>>
where
    I: TaskInput,
    O: TaskOutput,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input.validate()?;
    let start = Instant::now();
    let futures = input
        .tasks
        .into_iter()
        .map(&execute_one)
        .collect::<Vec<_>>();
    let raw_results: Vec<Result<O>> = join_all(futures).await;

    let mut results: Vec<O> = Vec::with_capacity(raw_results.len());
    let mut total_success = 0usize;
    let mut total_failed = 0usize;
    for (i, item) in raw_results.into_iter().enumerate() {
        match item {
            Ok(out) => {
                if out.is_success() {
                    total_success += 1;
                } else {
                    total_failed += 1;
                    if input.failure_strategy == FailureStrategy::FailFast {
                        let reason = out.error().unwrap_or("task reported failure").to_string();
                        results.push(out);
                        return Err(Error::PatternStopped {
                            position: i.to_string(),
                            reason,
                        });
                    }
                }
                results.push(out);
            }
            Err(e) => {
                total_failed += 1;
                if input.failure_strategy == FailureStrategy::FailFast {
                    return Err(Error::PatternStopped {
                        position: i.to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        }
    }

    Ok(ParallelOutput {
        results,
        total_success,
        total_failed,
        total_duration: start.elapsed(),
    })
}

// ---------------------------------------------------------------------------
// Loop
// ---------------------------------------------------------------------------

/// Iterate over `input.items`, calling the substitutor to produce a
/// concrete input per iteration, and dispatching. Sequential when
/// `parallel = false`, parallel otherwise. Named `run_loop` because
/// `loop` is a Rust keyword.
pub async fn run_loop<F, Fut, I, O>(
    input: LoopInput<I>,
    substitutor: Substitutor<I>,
    execute_one: F,
) -> Result<LoopOutput<O>>
where
    I: TaskInput,
    O: TaskOutput,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input.validate()?;
    let start = Instant::now();
    let no_params: HashMap<String, String> = HashMap::new();
    let item_count = input.items.len();
    let template = input.template;
    let strategy = input.failure_strategy;

    let inputs: Vec<I> = input
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| substitutor(&template, item.as_str(), i, &no_params))
        .collect();

    let outcomes = run_iterations(inputs, input.parallel, strategy, execute_one).await?;
    let (results, total_success, total_failed) = outcomes;

    Ok(LoopOutput {
        results,
        total_success,
        total_failed,
        total_duration: start.elapsed(),
        item_count,
    })
}

/// Cartesian-product loop. Substitutor receives an empty `item` string
/// and the per-combination parameter map.
pub async fn parameterized_loop<F, Fut, I, O>(
    input: ParameterizedLoopInput<I>,
    substitutor: Substitutor<I>,
    execute_one: F,
) -> Result<LoopOutput<O>>
where
    I: TaskInput,
    O: TaskOutput,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input.validate()?;
    let start = Instant::now();
    let combinations = generate_parameter_combinations(&input.parameters);
    let item_count = combinations.len();
    let template = input.template;
    let strategy = input.failure_strategy;

    let inputs: Vec<I> = combinations
        .into_iter()
        .enumerate()
        .map(|(i, params)| substitutor(&template, "", i, &params))
        .collect();

    let outcomes = run_iterations(inputs, input.parallel, strategy, execute_one).await?;
    let (results, total_success, total_failed) = outcomes;

    Ok(LoopOutput {
        results,
        total_success,
        total_failed,
        total_duration: start.elapsed(),
        item_count,
    })
}

async fn run_iterations<F, Fut, I, O>(
    inputs: Vec<I>,
    parallel_run: bool,
    failure_strategy: FailureStrategy,
    execute_one: F,
) -> Result<(Vec<O>, usize, usize)>
where
    I: TaskInput,
    O: TaskOutput,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    let mut results: Vec<O> = Vec::with_capacity(inputs.len());
    let mut total_success = 0usize;
    let mut total_failed = 0usize;

    let outcomes: Vec<Result<O>> = if parallel_run {
        let futures = inputs.into_iter().map(&execute_one).collect::<Vec<_>>();
        join_all(futures).await
    } else {
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(execute_one(input).await);
        }
        out
    };

    for (i, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            Ok(out) => {
                if out.is_success() {
                    total_success += 1;
                } else {
                    total_failed += 1;
                    if failure_strategy == FailureStrategy::FailFast {
                        let reason = out.error().unwrap_or("task reported failure").to_string();
                        results.push(out);
                        return Err(Error::PatternStopped {
                            position: i.to_string(),
                            reason,
                        });
                    }
                }
                results.push(out);
            }
            Err(e) => {
                total_failed += 1;
                if failure_strategy == FailureStrategy::FailFast {
                    return Err(Error::PatternStopped {
                        position: i.to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        }
    }

    Ok((results, total_success, total_failed))
}

// ---------------------------------------------------------------------------
// DAG
// ---------------------------------------------------------------------------

/// Execute a DAG. Nodes are dispatched in topological waves — every node
/// in a wave runs in parallel. Aborts on first failure if
/// `input.fail_fast` is set.
///
/// Named `run_dag` (not `dag`) so it doesn't collide with the internal
/// `dag` module that owns the input/output types.
pub async fn run_dag<F, Fut, I, O>(input: DAGInput<I>, execute_one: F) -> Result<DAGOutput<O>>
where
    I: TaskInput + Clone,
    O: TaskOutput + Clone,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O>>,
{
    input.validate()?;
    let start = Instant::now();
    let layers = input.topological_layers();
    let nodes = input.nodes;

    let mut results_map: HashMap<String, O> = HashMap::new();
    let mut node_results: Vec<NodeResult<O>> = Vec::with_capacity(nodes.len());
    let mut total_success = 0usize;
    let mut total_failed = 0usize;

    for layer in layers {
        let layer_inputs: Vec<(String, I)> = layer
            .iter()
            .map(|&idx| (nodes[idx].name.clone(), nodes[idx].input.clone()))
            .collect();
        let futures = layer_inputs
            .iter()
            .map(|(_, i)| execute_one(i.clone()))
            .collect::<Vec<_>>();
        let outcomes = join_all(futures).await;

        for ((name, _input), outcome) in layer_inputs.into_iter().zip(outcomes) {
            let node_start = Instant::now();
            match outcome {
                Ok(out) => {
                    let success = out.is_success();
                    let err_msg = out.error().map(str::to_string);
                    let cloned = out.clone();
                    if success {
                        total_success += 1;
                        results_map.insert(name.clone(), out);
                        node_results.push(NodeResult {
                            name,
                            result: Some(cloned),
                            error: None,
                            duration: node_start.elapsed(),
                            success: true,
                        });
                    } else {
                        total_failed += 1;
                        node_results.push(NodeResult {
                            name: name.clone(),
                            result: Some(cloned),
                            error: err_msg.clone(),
                            duration: node_start.elapsed(),
                            success: false,
                        });
                        if input.fail_fast {
                            return Err(Error::PatternStopped {
                                position: name,
                                reason: err_msg.unwrap_or_else(|| "task reported failure".into()),
                            });
                        }
                    }
                }
                Err(e) => {
                    total_failed += 1;
                    let msg = e.to_string();
                    node_results.push(NodeResult {
                        name: name.clone(),
                        result: None,
                        error: Some(msg.clone()),
                        duration: node_start.elapsed(),
                        success: false,
                    });
                    if input.fail_fast {
                        return Err(Error::PatternStopped {
                            position: name,
                            reason: msg,
                        });
                    }
                }
            }
        }
    }

    Ok(DAGOutput {
        results: results_map,
        node_results,
        total_success,
        total_failed,
        total_duration: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Step {
        id: u32,
        will_fail: bool,
    }
    impl TaskInput for Step {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct StepResult {
        id: u32,
        ok: bool,
        message: String,
    }
    impl TaskOutput for StepResult {
        fn is_success(&self) -> bool {
            self.ok
        }
        fn error(&self) -> Option<&str> {
            if self.ok { None } else { Some(&self.message) }
        }
    }

    fn ok(id: u32) -> Step {
        Step {
            id,
            will_fail: false,
        }
    }
    fn fail(id: u32) -> Step {
        Step {
            id,
            will_fail: true,
        }
    }

    async fn execute_step(step: Step) -> Result<StepResult> {
        if step.will_fail {
            Ok(StepResult {
                id: step.id,
                ok: false,
                message: format!("step {} failed", step.id),
            })
        } else {
            Ok(StepResult {
                id: step.id,
                ok: true,
                message: String::new(),
            })
        }
    }

    #[tokio::test]
    async fn execute_single_task() {
        let out: StepResult = execute(ok(1), execute_step).await.unwrap();
        assert!(out.is_success());
    }

    #[tokio::test]
    async fn pipeline_runs_all_when_continue() {
        let input = PipelineInput {
            tasks: vec![ok(1), fail(2), ok(3)],
            stop_on_error: false,
            cleanup: false,
        };
        let out: PipelineOutput<StepResult> = pipeline(input, execute_step).await.unwrap();
        assert_eq!(out.results.len(), 3);
        assert_eq!(out.total_success, 2);
        assert_eq!(out.total_failed, 1);
    }

    #[tokio::test]
    async fn pipeline_stops_when_stop_on_error() {
        let input = PipelineInput {
            tasks: vec![ok(1), fail(2), ok(3)],
            stop_on_error: true,
            cleanup: false,
        };
        let res = pipeline::<_, _, _, StepResult>(input, execute_step).await;
        match res {
            Err(Error::PatternStopped { position, .. }) => assert_eq!(position, "1"),
            other => panic!("expected PatternStopped, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn parallel_runs_all_when_continue() {
        let input = ParallelInput {
            tasks: vec![ok(1), fail(2), ok(3), ok(4)],
            failure_strategy: FailureStrategy::Continue,
        };
        let out: ParallelOutput<StepResult> = parallel(input, execute_step).await.unwrap();
        assert_eq!(out.results.len(), 4);
        assert_eq!(out.total_success, 3);
        assert_eq!(out.total_failed, 1);
    }

    #[tokio::test]
    async fn parallel_fail_fast_returns_first_failure() {
        let input = ParallelInput {
            tasks: vec![ok(1), fail(2), ok(3)],
            failure_strategy: FailureStrategy::FailFast,
        };
        let res = parallel::<_, _, _, StepResult>(input, execute_step).await;
        assert!(matches!(res, Err(Error::PatternStopped { .. })));
    }

    #[tokio::test]
    async fn loop_sequential_runs_per_item() {
        let counter = Arc::new(AtomicUsize::new(0));
        let input = LoopInput {
            items: vec!["a".into(), "b".into(), "c".into()],
            template: ok(0),
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
        };
        let counter_clone = counter.clone();
        let substitutor: Substitutor<Step> = Arc::new(move |template, _item, idx, _params| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Step {
                id: u32::try_from(idx).unwrap(),
                will_fail: template.will_fail,
            }
        });
        let out: LoopOutput<StepResult> = run_loop(input, substitutor, execute_step).await.unwrap();
        assert_eq!(out.item_count, 3);
        assert_eq!(out.total_success, 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn parameterized_loop_runs_cartesian_product() {
        let mut params: HashMap<String, Vec<String>> = HashMap::new();
        params.insert("a".into(), vec!["x".into(), "y".into()]);
        params.insert("b".into(), vec!["1".into(), "2".into(), "3".into()]);
        let input = ParameterizedLoopInput {
            parameters: params,
            template: ok(0),
            parallel: false,
            failure_strategy: FailureStrategy::Continue,
        };
        let substitutor: Substitutor<Step> = Arc::new(|_template, _item, idx, _params| Step {
            id: u32::try_from(idx).unwrap(),
            will_fail: false,
        });
        let out: LoopOutput<StepResult> = parameterized_loop(input, substitutor, execute_step)
            .await
            .unwrap();
        assert_eq!(out.item_count, 6);
        assert_eq!(out.total_success, 6);
    }

    #[tokio::test]
    async fn dag_runs_in_topological_layers() {
        use crate::dag::DAGNode;
        let nodes = vec![
            DAGNode {
                name: "a".into(),
                input: ok(1),
                dependencies: vec![],
            },
            DAGNode {
                name: "b".into(),
                input: ok(2),
                dependencies: vec!["a".into()],
            },
            DAGNode {
                name: "c".into(),
                input: ok(3),
                dependencies: vec!["a".into()],
            },
            DAGNode {
                name: "d".into(),
                input: ok(4),
                dependencies: vec!["b".into(), "c".into()],
            },
        ];
        let input = DAGInput {
            nodes,
            fail_fast: false,
            max_parallel: 0,
        };
        let out: DAGOutput<StepResult> = run_dag(input, execute_step).await.unwrap();
        assert_eq!(out.total_success, 4);
        assert_eq!(out.results.len(), 4);
        assert_eq!(out.node_results.len(), 4);
    }
}
