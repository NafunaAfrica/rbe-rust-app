//! RBE — Rich B Energy storefront.
//!
//! A single-binary web app: Axum serving server-rendered Maud/HTMX/Alpine pages,
//! an embedded SurrealDB, and outbound integrations with Shopify (catalog +
//! checkout) and Printify (print-on-demand fulfillment). A `rig`-powered site
//! agent is scaffolded in `agent.rs` for future admin automation.

mod agent;
mod auth;
mod config;
mod db;
mod error;
mod events;
mod models;
mod services;
mod state;
mod web;
mod webhooks;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (ignored in production where env is set directly).
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,rbe=debug")),
        )
        .init();

    let cfg = Config::from_env();

    // Embedded SurrealDB (SurrealKV) — created/seeded on first boot.
    let db = db::connect(&cfg).await?;
    tracing::info!(data_dir = %cfg.data_dir, "surrealdb ready");

    // Agent scaffold (rig). Logged so the operator knows if it's live.
    let site_agent = agent::SiteAgent::new(&cfg);
    tracing::info!(
        model = %cfg.agent_model,
        enabled = site_agent.enabled(),
        "site agent scaffolded (set ANTHROPIC_API_KEY to enable)"
    );

    let bind_addr = cfg.bind_addr.clone();
    let state = AppState::new(cfg, db);
    if let Err(err) = services::catalog_sync::sync_shopify_catalog(&state).await {
        tracing::warn!(error = %err, "shopify catalog sync skipped");
    } else {
        tracing::info!("shopify catalog synced into local storefront");
    }
    let app = web::router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("listening on http://{bind_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    // Draining done; give the embedded DB a beat to flush before the process
    // exits so a redeploy's stop doesn't kill SurrealKV mid-write.
    tracing::info!("server stopped; flushing embedded database");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(())
}

/// Resolve on SIGTERM (what `docker stop` sends on redeploy) or Ctrl-C, so the
/// server drains in-flight requests and stops writing before the process exits.
async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received; draining connections");
}
