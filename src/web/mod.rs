//! Web layer: HTML pages (Maud), the storefront/admin handlers, and the router.

pub mod account;
pub mod admin;
pub mod analytics;
pub mod auth_page;
pub mod blog;
pub mod charts;
pub mod home;
pub mod layout;
pub mod orders;
pub mod pages;
pub mod shop;
pub mod staff;

use axum::Router;
use axum::routing::{get, post};
use maud::{Markup, html};
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;

use crate::events;
use crate::state::AppState;
use crate::webhooks;

/// A tee "mockup" tile — the product image over its tee-color background.
/// (The reference app overlaid transparent SVG designs; those assets were
/// hosted on Lovable, so for now we show the local photo mockups.)
pub fn tee_mockup(image: &str, alt: &str, tee_color: &str) -> Markup {
    html! {
        div class="relative w-full overflow-hidden rounded-sm"
            style=(format!("aspect-ratio:1024/1280;background-color:{tee_color}")) {
            img src=(image) alt=(alt) loading="lazy"
                class="absolute inset-0 h-full w-full object-cover";
        }
    }
}

/// TEMPORARY diagnostic (build marker: diag-1). Reports how each table reads back
/// on the live volume + probes a specific email. No PII returned (emails masked to
/// domain + hash length only). Remove once the prod login issue is resolved.
async fn diag(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::Json<serde_json::Value> {
    use serde_json::json;
    let db = state.db();
    // Typed reads — exactly the path authenticate_* uses.
    let customer = match db.query("SELECT * FROM customer").await {
        Ok(mut r) => match r.take::<Vec<crate::models::Customer>>(0) {
            Ok(v) => json!({ "n": v.len() as i64, "status": "ok" }),
            Err(e) => json!({ "n": -1, "status": format!("take-err: {e}") }),
        },
        Err(e) => json!({ "n": -1, "status": format!("query-err: {e}") }),
    };
    let staff = match db.query("SELECT * FROM staff").await {
        Ok(mut r) => match r.take::<Vec<crate::models::Staff>>(0) {
            Ok(v) => json!({ "n": v.len() as i64, "emails": v.iter().map(|s| s.email.clone()).collect::<Vec<_>>(), "status": "ok" }),
            Err(e) => json!({ "n": -1, "status": format!("take-err: {e}") }),
        },
        Err(e) => json!({ "n": -1, "status": format!("query-err: {e}") }),
    };
    let product = match db.query("SELECT * FROM product").await {
        Ok(mut r) => match r.take::<Vec<crate::models::Product>>(0) {
            Ok(v) => json!({ "n": v.len() as i64, "status": "ok" }),
            Err(e) => json!({ "n": -1, "status": format!("take-err: {e}") }),
        },
        Err(e) => json!({ "n": -1, "status": format!("query-err: {e}") }),
    };

    let mut probe = json!(null);
    if let Some(email) = q.get("email") {
        let em = email.trim().to_lowercase();
        let all: Vec<crate::models::Customer> = match db.query("SELECT * FROM customer").await {
            Ok(mut r) => r.take(0).unwrap_or_default(),
            Err(_) => vec![],
        };
        let found = all.iter().find(|c| c.email == em);
        probe = json!({
            "select_star_total": all.len() as i64,
            "found": found.is_some(),
            "password_hash_len": found.map(|c| c.password_hash.len() as i64),
        });
    }

    axum::Json(json!({
        "build": "diag-2-typed",
        "counts": { "customer": customer, "staff": staff, "product": product },
        "probe": probe,
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        // Health check (Coolify probes this inside the container)
        .route("/health", get(|| async { "ok" }))
        // Storefront
        .route("/", get(home::home))
        .route("/shop", get(shop::shop_index))
        .route("/shop/{slug}", get(shop::product_detail))
        .route("/manifesto", get(pages::manifesto))
        .route("/journal", get(blog::journal))
        .route("/journal/{slug}", get(blog::article))
        .route("/api/checkout", post(shop::checkout))
        .route("/api/_diag", get(diag))
        .route("/events", get(events::events))
        // Customer accounts
        .route("/account", get(account::account_page))
        .route("/account/register", get(account::register_page).post(account::register_submit))
        .route("/account/login", get(account::login_page).post(account::login_submit))
        .route("/account/logout", get(account::logout))
        // Auth
        .route("/auth", get(auth_page::login_page).post(auth_page::login_submit))
        .route("/auth/logout", get(auth_page::logout))
        // Owner business dashboard (admin or owner)
        .route("/dashboard", get(staff::dashboard_owner))
        .route("/dashboard/orders", get(orders::orders_list))
        .route("/dashboard/posts", get(blog::posts_list).post(blog::post_save))
        .route("/dashboard/posts/new", get(blog::post_new))
        .route("/dashboard/posts/{slug}/edit", get(blog::post_edit))
        // Admin (guarded by the AdminUser extractor inside each handler)
        .route("/admin", get(admin::dashboard))
        .route("/admin/team", get(staff::team_page).post(staff::team_create))
        .route("/admin/printify", get(admin::printify_page))
        .route("/admin/printify/sync", get(admin::printify_sync_stream))
        // Webhooks
        .route("/api/webhooks/printify", post(webhooks::printify))
        .route("/api/webhooks/shopify", post(webhooks::shopify))
        // Static assets (CSS, vendored JS, images, favicon)
        .nest_service("/static", ServeDir::new("static"))
        // First-party page-view analytics (records public GET page loads)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            analytics::track_pageviews,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
