use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

use harmony::config::Config;
use harmony::storage::Storage;
use harmony::{AppState, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "harmony=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let storage = Storage::from_spec(&config.storage_spec)?;
    info!("loading data from {}", storage.describe());
    let (data, etag) = storage.load().await?;
    info!(
        projects = data.projects.len(),
        tasks = data.tasks.len(),
        sessions = data.sessions.len(),
        "loaded"
    );

    let state = Arc::new(AppState::new(data, etag, storage));
    let listener = TcpListener::bind(config.bind).await?;
    info!("listening on http://{}", config.bind);
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutting down");
        })
        .await?;
    Ok(())
}
