//! Signal-driven graceful shutdown future.

use tokio::signal;

/// Future that resolves when the process receives SIGINT (Ctrl-C) on all
/// platforms, or SIGTERM on Unix.
///
/// Use this with [`crate::Server::run_with_shutdown`] for custom shutdown
/// orchestration, or rely on [`crate::Server::run`] which installs this
/// automatically.
///
/// ```no_run
/// # async fn run() {
/// altair_server::shutdown_signal().await;
/// println!("shutdown signal received");
/// # }
/// ```
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::warn!("failed to install Ctrl-C handler: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("failed to install SIGTERM handler: {e}");
                // Block forever — the ctrl_c branch will still resolve.
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_signal_can_race_against_timer() {
        // We can't trigger a signal in a unit test, but we can verify that
        // the future is well-formed by racing it against an immediate
        // tokio::time::sleep — the sleep should win, proving shutdown_signal
        // hasn't already completed.
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), shutdown_signal()).await;
        assert!(
            result.is_err(),
            "shutdown_signal should not complete in 50ms"
        );
    }
}
