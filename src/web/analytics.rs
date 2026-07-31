//! First-party analytics: a middleware that records page views, and the
//! dashboard section that summarises traffic + sales as SVG charts.

use std::collections::{HashMap, HashSet};

use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use maud::{Markup, html};
use serde::Deserialize;

use crate::error::AppResult;
use crate::state::AppState;

use super::charts::bar_chart;

// ---------------------------------------------------------------------------
// Tracking middleware
// ---------------------------------------------------------------------------

/// Track GET views of public storefront pages. Sets a long-lived `rbe_sid`
/// cookie the first time so we can count unique visitors without accounts.
pub async fn track_pageviews(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let is_get = req.method() == axum::http::Method::GET;
    let path = req.uri().path().to_string();
    let referrer = req
        .headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let existing_sid = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| cookie_value(c, "rbe_sid"));
    let sid = existing_sid
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut res = next.run(req).await;

    if is_get && trackable(&path) {
        let db = state.db().clone();
        let (p, s, r) = (path, sid.clone(), referrer);
        tokio::spawn(async move {
            if let Err(e) = crate::db::record_pageview(&db, &p, &s, r.as_deref()).await {
                tracing::error!(error = %e, path = %p, "record_pageview failed");
            }
        });
        if existing_sid.is_none() {
            if let Ok(v) = HeaderValue::from_str(&format!(
                "rbe_sid={sid}; Path=/; Max-Age=31536000; SameSite=Lax"
            )) {
                res.headers_mut().append(header::SET_COOKIE, v);
            }
        }
    }
    res
}

fn trackable(p: &str) -> bool {
    p == "/"
        || p == "/shop"
        || p.starts_with("/shop/")
        || p == "/manifesto"
        || p == "/journal"
        || p.starts_with("/journal/")
}

fn cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    cookie_header
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == name)
        .map(|(_, v)| v.trim().to_string())
}

// ---------------------------------------------------------------------------
// Dashboard analytics
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PageviewRow {
    #[serde(default)]
    session: String,
    #[serde(default)]
    day: String,
}

#[derive(Deserialize)]
struct OrderAgg {
    #[serde(default)]
    total: String,
}

/// Build the tiles + 14-day views chart shown on the dashboard.
pub async fn dashboard_section(state: &AppState) -> AppResult<Markup> {
    // Build the last 14 day keys (YYYY-MM-DD) and short labels (MM-DD).
    let today = time::OffsetDateTime::now_utc().date();
    let mut days: Vec<(String, String)> = Vec::with_capacity(14);
    for i in (0..14).rev() {
        let d = today - time::Duration::days(i);
        let key = format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day());
        let label = format!("{:02}-{:02}", u8::from(d.month()), d.day());
        days.push((key, label));
    }
    let since = days.first().map(|(k, _)| k.clone()).unwrap_or_default();

    let views: Vec<PageviewRow> = state
        .db()
        .query("SELECT session, day FROM pageview WHERE day >= $since")
        .bind(("since", since))
        .await?
        .take(0)?;
    let orders: Vec<OrderAgg> = state.db().query("SELECT total FROM order").await?.take(0)?;
    let currency: Vec<String> = state
        .db()
        .query("SELECT VALUE currency FROM order LIMIT 1")
        .await?
        .take(0)?;
    let currency = currency.into_iter().next().unwrap_or_else(|| "USD".into());

    // Aggregate.
    let total_views = views.len();
    let visitors = views.iter().map(|v| v.session.as_str()).collect::<HashSet<_>>().len();
    let orders_count = orders.len();
    let revenue: f64 = orders.iter().filter_map(|o| o.total.parse::<f64>().ok()).sum();

    let mut per_day: HashMap<&str, f64> = HashMap::new();
    for v in &views {
        *per_day.entry(v.day.as_str()).or_insert(0.0) += 1.0;
    }
    let chart_data: Vec<(String, f64)> = days
        .iter()
        .map(|(key, label)| (label.clone(), *per_day.get(key.as_str()).unwrap_or(&0.0)))
        .collect();

    Ok(html! {
        div class="grid grid-cols-2 gap-4 sm:grid-cols-4" {
            (tile("Revenue", &format!("{currency} {revenue:.0}")))
            (tile("Orders", &orders_count.to_string()))
            (tile("Visitors · 14d", &visitors.to_string()))
            (tile("Page views · 14d", &total_views.to_string()))
        }
        div class="mt-6 rounded-lg border border-ink/10 bg-white p-5" {
            div class="mb-3 flex items-center justify-between" {
                div class="text-sm font-semibold" { "Traffic" }
                div class="text-xs uppercase tracking-widest opacity-50" { "Page views · last 14 days" }
            }
            (bar_chart(&chart_data, "var(--hot)"))
        }
    })
}

fn tile(label: &str, value: &str) -> Markup {
    html! {
        div class="rounded-lg border border-ink/10 bg-white p-4" {
            div class="font-display text-3xl text-[color:var(--hot)] tabular-nums" { (value) }
            div class="mt-1 text-xs uppercase tracking-widest opacity-60" { (label) }
        }
    }
}
