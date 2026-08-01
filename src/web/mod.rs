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
        .route("/admin/products", get(admin::products_page).post(admin::product_save))
        .route("/admin/products/new", get(admin::product_new))
        .route("/admin/products/{slug}/edit", get(admin::product_edit))
        .route("/admin/products/delete", post(admin::product_delete))
        .route("/admin/shopify", get(admin::shopify_page))
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
