//! End-to-end behavior tests for altair-concurrent.

use altair_concurrent::prelude::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[tokio::test]
async fn three_parallel_tasks_share_results() {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("alpha", |_| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, std::io::Error>("a".to_string())
        })
        .insert("beta", |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok::<_, std::io::Error>("b".to_string())
        })
        .insert("gamma", |_| async {
            Ok::<_, std::io::Error>("g".to_string())
        });

    let results = execute_concurrently(tasks).await.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results["alpha"], "a");
    assert_eq!(results["beta"], "b");
    assert_eq!(results["gamma"], "g");
}

#[tokio::test]
async fn cancellation_token_propagates_to_tasks() {
    let token = CancellationToken::new();
    let m: TaskMap<bool> = TaskMap::new().insert("respect_ct", |ct| async move {
        tokio::select! {
            () = ct.cancelled() => Ok::<_, std::io::Error>(false),
            () = tokio::time::sleep(Duration::from_secs(10)) => Ok::<_, std::io::Error>(true),
        }
    });

    let inner = token.clone();
    let handle =
        tokio::spawn(async move { execute_concurrently(m).with_cancellation(inner).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    token.cancel();
    let result = handle.await.unwrap();
    // External cancel may yield either flavor; success means the task observed the token.
    assert!(result.is_err() || !result.unwrap()["respect_ct"]);
}
