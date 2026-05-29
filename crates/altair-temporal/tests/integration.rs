//! Integration test scaffold.
//!
//! Gated behind `integration-tests` feature + Linux only. Body is a
//! placeholder pending stabilisation of the temporalio Rust SDK. Run
//! with `cargo test -p altair-temporal --features integration-tests`.

#![cfg(all(feature = "integration-tests", target_os = "linux"))]

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_runs_a_workflow() {
    // TODO: spin up Temporal server via testcontainers, build a worker,
    // register a minimal workflow + activity, run a workflow to completion.
    // Currently a scaffold; flesh out once SDK API stabilises post-0.4.
    panic!("integration test scaffold — pending SDK stabilisation");
}
