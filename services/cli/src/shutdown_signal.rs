use tokio_util::sync::CancellationToken;

pub async fn shutdown_signal(cancellation_token: CancellationToken) {
    let ctrl_c = async { tokio::signal::ctrl_c().await };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => match signal.recv().await {
                Some(_) => Ok(()),
                None => Ok(()),
            },
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
        Ok(_) => log::info!("shutdown signal received, shutting down"),
        Err(e) => log::error!("error receiving shutdown signal: {}", e),
    }

    cancellation_token.cancel();
}
