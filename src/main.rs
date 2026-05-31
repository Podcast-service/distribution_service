use std::time::Duration;

use anyhow::Context;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::Level;

mod auth_client;
mod categories;
mod config;
mod db;
mod error;
mod models;
mod routes;
mod rss;
mod state;
mod telemetry;

use auth_client::AuthClient;
use config::AppConfig;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let _telemetry = telemetry::init("distribution_service");

    let config = AppConfig::from_env().context("invalid configuration")?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .acquire_timeout(Duration::from_secs(config.db_connect_timeout_seconds))
        .connect(&config.database_url)
        .await
        .context("failed to connect to Postgres")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("database ping failed")?;

    let auth = AuthClient::new(
        config.auth_service_url.clone(),
        config.auth_internal_api_token.clone(),
        Duration::from_secs(config.auth_cache_ttl_seconds),
    );

    let state = AppState {
        pool,
        public_base_url: config.public_base_url.clone(),
        auth,
    };

    let app: Router = routes::router(state);

    let listener = TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_addr))?;

    tracing::info!(addr = %config.bind_addr, "distribution_service listening");

    axum::serve(
        listener,
        app.layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::new().level(Level::INFO))),
    )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
