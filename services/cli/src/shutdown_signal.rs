use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

pub async fn shutdown_signal(cancellation_token: CancellationToken) {
    let ctrl_c = async { ctrl_c().await };

    #[cfg(unix)]
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
                Ok(())
            }
            Err(e) => Err(e),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let result = tokio::select! {
      result = ctrl_c => result,
      result = terminate => result,
    };

    match result {
        Ok(()) => log::info!("shutdown signal received, shutting down"),
        Err(e) => log::error!("error receiving shutdown signal: {e}"),
    }

    cancellation_token.cancel();
}
