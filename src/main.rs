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
    let app = web::router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("listening on http://{bind_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
