use std::sync::Arc;
use tokio::{net::TcpListener, signal};
use wattlab_api::{
    adapters::outbound::postgres::{DatabaseConfig, PgSessionRepository, connect_and_migrate},
    build_app,
    config::Config,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).init();
    let config = Config::load().map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
    })?;
    let database_config = DatabaseConfig::load().map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
    })?;
    let pool = connect_and_migrate(&database_config).await?;
    let listener = TcpListener::bind(config.address).await?;
    tracing::info!(address = %listener.local_addr()?, "WattLab API listening");
    let app = build_app(
        config.api_token,
        Arc::new(PgSessionRepository::new(pool.clone())),
    );
    let streams = app.shutdown.clone();
    let result = axum::serve(listener, app.router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            streams.trigger();
        })
        .await;
    pool.close().await;
    result?;
    tracing::info!("WattLab API stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::error!(%error, "Cannot register Ctrl+C handler; shutting down");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(%error, "Cannot register SIGTERM handler; shutting down"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("Shutdown requested");
}
