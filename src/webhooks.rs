//! Inbound webhooks from Printify and Shopify. Each verifies an HMAC signature,
//! bumps the shop-cache version, and broadcasts a cache-bust event.
//!
//! Ported from the reference `src/routes/api/public/webhooks.*.ts`.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::db;
use crate::state::{AppState, CacheEvent};

type HmacSha256 = Hmac<Sha256>;

/// Printify: `X-Pfy-Signature: sha256=<hex>`, HMAC-SHA256 hex digest.
pub async fn printify(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, &'static str) {
    let secret = match &state.cfg().printify_webhook_secret {
        Some(s) => s.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "Webhook secret not configured"),
    };

    let raw = headers.get("x-pfy-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    let sig = raw.strip_prefix("sha256=").unwrap_or(raw);
    let expected = hmac_hex(secret.as_bytes(), &body);
    if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "Invalid signature");
    }

    let event = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
        .unwrap_or_else(|| "unknown".into());

    bump(&state, &format!("printify:{event}")).await;
    (StatusCode::OK, "ok")
}

/// Shopify: `X-Shopify-Hmac-Sha256`, HMAC-SHA256 base64 digest.
pub async fn shopify(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, &'static str) {
    let secret = match &state.cfg().shopify_webhook_secret {
        Some(s) => s.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "Webhook secret not configured"),
    };

    let sig = headers.get("x-shopify-hmac-sha256").and_then(|v| v.to_str().ok()).unwrap_or("");
    let expected = hmac_base64(secret.as_bytes(), &body);
    if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "Invalid signature");
    }

    let topic = headers.get("x-shopify-topic").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
    bump(&state, &format!("shopify:{topic}")).await;
    (StatusCode::OK, "ok")
}

async fn bump(state: &AppState, source: &str) {
    match db::bump_version(state.db(), source).await {
        Ok(version) => {
            let _ = state.events().send(CacheEvent { version, source: source.to_string() });
        }
        Err(e) => tracing::error!(error = %e, "failed to bump shop_cache version"),
    }
}

fn hmac_hex(key: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

fn hmac_base64(key: &[u8], body: &[u8]) -> String {
    use base64::Engine;
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(body);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}
