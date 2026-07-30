//! Shared application state handed to every request handler.

use std::sync::Arc;

use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use tokio::sync::broadcast;

use crate::config::Config;

/// Cache-bust signal broadcast to connected browsers (the SurrealDB-backed
/// replacement for the old Supabase Realtime `shop_cache` subscription).
#[derive(Debug, Clone)]
pub struct CacheEvent {
    pub version: i64,
    pub source: String,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub cfg: Config,
    pub db: Surreal<Db>,
    pub http: reqwest::Client,
    pub events: broadcast::Sender<CacheEvent>,
}

impl AppState {
    pub fn new(cfg: Config, db: Surreal<Db>) -> Self {
        let (events, _) = broadcast::channel(64);
        let http = reqwest::Client::builder()
            .user_agent("rbe/0.1 (+https://rbe.club)")
            .build()
            .expect("failed to build http client");
        AppState(Arc::new(Inner {
            cfg,
            db,
            http,
            events,
        }))
    }

    pub fn cfg(&self) -> &Config {
        &self.0.cfg
    }
    pub fn db(&self) -> &Surreal<Db> {
        &self.0.db
    }
    pub fn http(&self) -> &reqwest::Client {
        &self.0.http
    }
    pub fn events(&self) -> &broadcast::Sender<CacheEvent> {
        &self.0.events
    }
}
