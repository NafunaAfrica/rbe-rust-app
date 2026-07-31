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

    // Order events are ingested into our own DB (powers the dashboard & customer
    // order history). Product/inventory events just bust the storefront cache.
    if topic.starts_with("orders/") {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) {
            if let Some(order) = parse_shopify_order(&v) {
                if let Err(e) = db::upsert_order(state.db(), &order).await {
                    tracing::error!(error = %e, "failed to ingest shopify order");
                }
            }
        }
    } else {
        bump(&state, &format!("shopify:{topic}")).await;
    }
    (StatusCode::OK, "ok")
}

/// Map a Shopify order webhook payload onto our `Order` model. Tracking is read
/// from the first fulfilment that carries it (Printify ships → Shopify records
/// the tracking → this webhook delivers it).
fn parse_shopify_order(v: &serde_json::Value) -> Option<crate::models::Order> {
    use crate::models::{Order, OrderLine};
    let id = v.get("id")?;
    let shopify_order_id = match id {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => return None,
    };

    let str_of = |key: &str| v.get(key).and_then(|x| x.as_str()).map(String::from);

    let line_items = v
        .get("line_items")
        .and_then(|l| l.as_array())
        .map(|arr| {
            arr.iter()
                .map(|li| OrderLine {
                    title: li.get("title").and_then(|t| t.as_str()).unwrap_or("Item").to_string(),
                    quantity: li.get("quantity").and_then(|q| q.as_i64()).unwrap_or(1),
                    price: li.get("price").and_then(|p| p.as_str()).map(String::from),
                })
                .collect()
        })
        .unwrap_or_default();

    // First fulfilment carrying tracking info.
    let fulfilment = v
        .get("fulfillments")
        .and_then(|f| f.as_array())
        .and_then(|arr| arr.iter().find(|f| f.get("tracking_number").is_some()));
    let tracking_number = fulfilment.and_then(|f| f.get("tracking_number").and_then(|t| t.as_str())).map(String::from);
    let tracking_url = fulfilment
        .and_then(|f| {
            f.get("tracking_url")
                .and_then(|t| t.as_str())
                .or_else(|| f.get("tracking_urls").and_then(|u| u.as_array()).and_then(|a| a.first()).and_then(|u| u.as_str()))
        })
        .map(String::from);

    Some(Order {
        shopify_order_id,
        number: str_of("name"),
        email: str_of("email").or_else(|| str_of("contact_email")),
        currency: str_of("currency").unwrap_or_else(|| "USD".into()),
        total: str_of("total_price").unwrap_or_default(),
        financial_status: str_of("financial_status"),
        fulfillment_status: str_of("fulfillment_status"),
        line_items,
        tracking_url,
        tracking_number,
        created_at: str_of("created_at"),
    })
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
