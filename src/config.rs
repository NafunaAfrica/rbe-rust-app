//! Runtime configuration, loaded from environment variables (see `.env.example`).

use std::env;

#[derive(Clone)]
pub struct Config {
    /// Socket address the HTTP server binds to, e.g. `0.0.0.0:8080`.
    pub bind_addr: String,
    /// Directory where the embedded SurrealDB (SurrealKV) persists its data.
    pub data_dir: String,
    /// Public origin of the deployment, used when handing image URLs to Printify.
    pub public_base_url: String,

    /// Secret used to sign admin session JWTs.
    pub jwt_secret: String,
    /// The single admin account (email + password) that can reach `/admin`.
    pub admin_email: String,
    pub admin_password: String,

    // --- External commerce integrations ---
    pub shopify_store_domain: String,
    pub shopify_storefront_token: String,
    pub shopify_api_version: String,
    pub shopify_webhook_secret: Option<String>,

    pub printify_api_token: Option<String>,
    pub printify_webhook_secret: Option<String>,

    // --- Agent (rig) ---
    pub anthropic_api_key: Option<String>,
    pub agent_model: String,
}

fn var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            bind_addr: var("RBE_BIND_ADDR", "0.0.0.0:8080"),
            data_dir: var("RBE_DATA_DIR", "./data/surreal"),
            public_base_url: var("RBE_PUBLIC_BASE_URL", "http://localhost:8080"),

            jwt_secret: var("RBE_JWT_SECRET", "dev-only-insecure-change-me"),
            admin_email: var("RBE_ADMIN_EMAIL", "admin@rbe.club"),
            admin_password: var("RBE_ADMIN_PASSWORD", "changeme"),

            shopify_store_domain: var("SHOPIFY_STORE_DOMAIN", "f0epji-nd.myshopify.com"),
            shopify_storefront_token: var(
                "SHOPIFY_STOREFRONT_TOKEN",
                "f57a26bebd7fd009ce29dd16b8b79096",
            ),
            shopify_api_version: var("SHOPIFY_API_VERSION", "2025-07"),
            shopify_webhook_secret: opt("SHOPIFY_WEBHOOK_SECRET"),

            printify_api_token: opt("PRINTIFY_API_TOKEN"),
            printify_webhook_secret: opt("PRINTIFY_WEBHOOK_SECRET"),

            anthropic_api_key: opt("ANTHROPIC_API_KEY"),
            agent_model: var("RBE_AGENT_MODEL", "claude-sonnet-5"),
        }
    }
}
