//! Admin control panel: website management, Shopify visibility, Printify sync
//! (legacy for now), and team access.

use axum::extract::{Query, State};
use axum::response::Html;
use axum::response::sse::{Event, KeepAlive, Sse};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::AdminUser;
use crate::error::AppResult;
use crate::models::{Post, Product};
use crate::services::printify::Printify;
use crate::services::shopify::{Product as ShopifyProduct, Shopify};
use crate::state::AppState;

use super::layout::{Nav, shell};

#[derive(Deserialize)]
struct CountRow {
    count: usize,
}

#[derive(Default)]
struct DashboardStats {
    products: usize,
    posts: usize,
    published_posts: usize,
    orders: usize,
    live_shopify_products: usize,
}

pub async fn dashboard(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let stats = load_dashboard_stats(&state).await?;

    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-16" {
            div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Website admin" }
                    h1 class="mt-2 font-display text-5xl md:text-6xl" { "Manage the site" }
                    p class="mt-3 max-w-2xl text-sm opacity-70" {
                        "One place to manage the storefront, content, orders, team access, and the Shopify connection that powers the bag and checkout."
                    }
                }
                a href="/auth/logout" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "Sign out" }
            }

            div class="mt-10 grid gap-4 sm:grid-cols-2 xl:grid-cols-5" {
                (stat("Catalog items", &stats.products.to_string()))
                (stat("Visible in Shopify", &stats.live_shopify_products.to_string()))
                (stat("Orders", &stats.orders.to_string()))
                (stat("Journal posts", &stats.posts.to_string()))
                (stat("Published posts", &stats.published_posts.to_string()))
            }

            div class="mt-10 grid gap-4 lg:grid-cols-3" {
                (admin_card(
                    "Shopify visibility",
                    "Check whether the current storefront token can see the products your bag and checkout rely on.",
                    "/admin/shopify",
                ))
                (admin_card(
                    "Journal manager",
                    "Write, edit, and publish website content for the journal.",
                    "/dashboard/posts",
                ))
                (admin_card(
                    "Orders and fulfilment",
                    "Review incoming Shopify orders and shipment status from your fulfilment flow.",
                    "/dashboard/orders",
                ))
                (admin_card(
                    "Team access",
                    "Create owner accounts and control who can sign in to manage the site.",
                    "/admin/team",
                ))
                (admin_card(
                    "Storefront",
                    "Open the public shop and verify the customer-facing experience.",
                    "/shop",
                ))
                (admin_card(
                    "Legacy print sync",
                    "The old Printify sync is still here while you transition to the new print provider.",
                    "/admin/printify",
                ))
            }

            div class="mt-10 rounded-xl border border-ink/10 bg-white p-6" {
                h2 class="font-display text-3xl" { "What this covers" }
                ul class="mt-4 space-y-3 text-sm opacity-80" {
                    li { "Storefront health: whether Shopify can actually see the handles this app sends to checkout." }
                    li { "Content management: journal publishing is already wired into the dashboard." }
                    li { "Operations: order history and fulfilment tracking come back through Shopify webhooks." }
                    li { "Access control: admins can create owner accounts from the team screen." }
                }
            }
        }
    };

    Ok(Html(shell("Admin - RBE", "RBE website admin dashboard.", Nav::None, body).into_string()))
}

fn admin_card(title: &str, desc: &str, href: &str) -> Markup {
    html! {
        a href=(href) class="block rounded-lg border border-ink/10 bg-white p-6 transition hover:border-[color:var(--hot)] hover:shadow-lg hover:shadow-[color:var(--hot)]/10" {
            div class="font-display text-2xl" { (title) }
            p class="mt-2 text-sm opacity-70" { (desc) }
        }
    }
}

fn stat(label: &str, value: &str) -> Markup {
    html! {
        div class="rounded-lg border border-ink/10 bg-white p-4" {
            div class="text-3xl font-semibold text-[color:var(--hot)]" { (value) }
            div class="mt-1 text-xs uppercase tracking-widest opacity-60" { (label) }
        }
    }
}

async fn load_dashboard_stats(state: &AppState) -> AppResult<DashboardStats> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;
    let posts: Vec<Post> = state
        .db()
        .query("SELECT * FROM post")
        .await?
        .take(0)?;
    let order_rows: Vec<CountRow> = state
        .db()
        .query("SELECT count() AS count FROM order GROUP ALL")
        .await?
        .take(0)?;

    let shopify = Shopify::new(state.cfg(), state.http());
    let mut live_shopify_products = 0;
    for product in &products {
        if shopify.product_by_handle(product.storefront_handle()).await?.is_some() {
            live_shopify_products += 1;
        }
    }

    Ok(DashboardStats {
        products: products.len(),
        posts: posts.len(),
        published_posts: posts.iter().filter(|p| p.is_published()).count(),
        orders: order_rows.first().map(|r| r.count).unwrap_or(0),
        live_shopify_products,
    })
}

struct ProductVisibility {
    product: Product,
    shopify: ShopifyProduct,
}

pub async fn shopify_page(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;

    let shopify = Shopify::new(state.cfg(), state.http());
    let shop = shopify.shop_info().await?;
    let app_scopes = shopify.current_app_scopes().await.unwrap_or_default();

    let mut visible: Vec<ProductVisibility> = Vec::new();
    let mut missing: Vec<Product> = Vec::new();
    for product in products {
        match shopify.product_by_handle(product.storefront_handle()).await? {
            Some(found) => visible.push(ProductVisibility {
                product,
                shopify: found,
            }),
            None => missing.push(product),
        }
    }

    let visible_count = visible.len();
    let missing_count = missing.len();
    let likely_wrong_token = visible_count == 0;
    let no_app_scopes = app_scopes.is_empty();

    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-16" {
            div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Shopify diagnostics" }
                    h1 class="mt-2 font-display text-5xl" { "Storefront visibility" }
                    p class="mt-2 text-sm opacity-70" {
                        "This uses the exact Storefront API token configured in the app, so it reflects what the bag and checkout can see right now."
                    }
                }
                a href="/admin" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- Admin" }
            }

            div class="mt-8 grid gap-4 md:grid-cols-3" {
                (stat("Visible", &visible_count.to_string()))
                (stat("Missing", &missing_count.to_string()))
                (stat(
                    "Currencies",
                    &shop.payment_settings.enabled_presentment_currencies.join(", "),
                ))
            }

            div class="mt-8 rounded-xl border border-ink/10 bg-white p-6" {
                dl class="grid gap-4 text-sm md:grid-cols-2" {
                    div {
                        dt class="text-xs uppercase tracking-widest opacity-60" { "Store" }
                        dd class="mt-1 font-medium" { (shop.name) }
                    }
                    div {
                        dt class="text-xs uppercase tracking-widest opacity-60" { "Primary domain" }
                        dd class="mt-1 font-medium break-all" { (shop.primary_domain.url) }
                    }
                    div {
                        dt class="text-xs uppercase tracking-widest opacity-60" { "Configured domain" }
                        dd class="mt-1 font-medium break-all" { (state.cfg().shopify_store_domain.clone()) }
                    }
                    div {
                        dt class="text-xs uppercase tracking-widest opacity-60" { "Shipping countries" }
                        dd class="mt-1 font-medium" { (format!("{}", shop.ships_to_countries.len())) }
                    }
                }
            }

            div class="mt-8 rounded-xl border border-ink/10 bg-white p-6" {
                h2 class="font-display text-3xl" { "App access" }
                p class="mt-2 text-sm opacity-70" {
                    "These are the scopes currently granted to the Dev Dashboard app installation."
                }
                @if no_app_scopes {
                    p class="mt-4 text-sm text-amber-700" {
                        "This app currently has no granted scopes. Until Shopify grants product and storefront-related scopes, checkout lookups will stay blocked."
                    }
                } @else {
                    ul class="mt-4 grid gap-2 text-sm md:grid-cols-2" {
                        @for scope in &app_scopes {
                            li class="rounded-md bg-black/5 px-3 py-2 font-mono text-xs" { (scope) }
                        }
                    }
                }
            }

            @if likely_wrong_token {
                div class="mt-8 rounded-xl border border-amber-300 bg-amber-50 p-5 text-sm text-amber-900" {
                    div class="font-semibold uppercase tracking-widest" { "Likely token mismatch" }
                    p class="mt-2" {
                        "The token is valid for the store, but it cannot see any storefront products. That usually means this token is tied to a different publication or sales channel than Online Store. A fresh Storefront API token created for this store is the cleanest fix."
                    }
                }
            }

            div class="mt-10 grid gap-6 lg:grid-cols-2" {
                div class="rounded-xl border border-ink/10 bg-white p-6" {
                    h2 class="font-display text-3xl" { "Visible to checkout" }
                    p class="mt-2 text-sm opacity-70" { "These handles resolve through the current token." }
                    ul class="mt-4 divide-y" {
                        @if visible.is_empty() {
                            li class="py-3 text-sm opacity-60" { "No products are visible yet." }
                        }
                        @for row in &visible {
                            li class="py-3" {
                                div class="flex items-start justify-between gap-4" {
                                    div {
                                        div class="font-medium" { (row.product.slogan_flat()) }
                                        div class="text-xs uppercase tracking-widest opacity-60" { (row.product.slug.clone()) }
                                        div class="text-[11px] opacity-50" { "Shopify handle: " (row.product.storefront_handle()) }
                                    }
                                    div class="text-right text-sm" {
                                        div class="text-green-700" { "Live" }
                                        @if let Some(first) = row.shopify.variants.0.first() {
                                            div class="text-xs opacity-60" {
                                                (first.price.currency_code.clone()) " " (first.price.amount.clone())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div class="rounded-xl border border-ink/10 bg-white p-6" {
                    h2 class="font-display text-3xl" { "Missing from token" }
                    p class="mt-2 text-sm opacity-70" { "These exist in the app database but do not resolve through the current token." }
                    ul class="mt-4 divide-y" {
                        @if missing.is_empty() {
                            li class="py-3 text-sm opacity-60" { "Everything in the app is visible to Shopify checkout." }
                        }
                        @for product in &missing {
                            li class="py-3 flex items-start justify-between gap-4" {
                                div {
                                    div class="font-medium" { (product.slogan_flat()) }
                                    div class="text-xs uppercase tracking-widest opacity-60" { (product.slug.clone()) }
                                    div class="text-[11px] opacity-50" { "Shopify handle: " (product.storefront_handle()) }
                                }
                                div class="text-sm text-amber-700" { "Not visible" }
                            }
                        }
                    }
                }
            }
        }
    };

    Ok(Html(shell(
        "Shopify Visibility - RBE Admin",
        "Check which products the current Shopify Storefront token can see.",
        Nav::None,
        body,
    ).into_string()))
}

pub async fn printify_page(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;

    // Shops require a Printify token; degrade gracefully if not configured.
    let printify = Printify::new(state.cfg(), state.http());
    let shops = printify.shops().await.unwrap_or_default();

    let synced = products.iter().filter(|p| p.printify_product_id.is_some()).count();

    let body = html! {
        div class="mx-auto max-w-3xl px-4 py-16" x-data="printifySync()" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Printify Sync" }
                a href="/admin" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- Admin" }
            }
            p class="mt-2 text-muted-foreground" { "Legacy Printify sync while the new print provider is being phased in." }

            div class="mt-8" {
                label class="text-sm font-medium" { "Printify shop" }
                @if shops.is_empty() {
                    p class="mt-2 text-sm opacity-60" { "No shops (set PRINTIFY_API_TOKEN to connect)." }
                } @else {
                    select "x-model"="shopId" class="mt-2 block w-full max-w-md rounded-md border border-ink/20 bg-white px-3 py-2 text-sm" {
                        @for s in &shops {
                            @let id = s.id.to_string().trim_matches('"').to_string();
                            option value=(id) {
                                (s.title) " - " (s.sales_channel.clone().unwrap_or_default()) " - id " (id)
                            }
                        }
                    }
                }
            }

            div class="mt-8 grid grid-cols-3 gap-4" {
                (stat("Total", &products.len().to_string()))
                (stat("Synced", &synced.to_string()))
                (stat("Pending", &(products.len() - synced).to_string()))
            }

            div class="mt-8 flex flex-wrap gap-3" {
                button "@click"="run('pending')" ":disabled"="running || !shopId"
                    class="rounded-full bg-[color:var(--hot)] px-5 py-2 text-sm font-semibold uppercase tracking-widest text-white disabled:opacity-40" {
                    "Sync pending"
                }
                button "@click"="run('all')" ":disabled"="running || !shopId"
                    class="rounded-full border border-ink/20 px-5 py-2 text-sm font-semibold uppercase tracking-widest disabled:opacity-40" {
                    "Sync all"
                }
                span x-show="running" class="self-center text-sm opacity-60" { "Running..." }
            }

            div class="mt-8" {
                h2 class="font-semibold" { "Products (" (products.len()) ")" }
                ul class="mt-2 divide-y" {
                    @for p in &products {
                        li class="flex items-center justify-between py-2" {
                            div {
                                div class="font-medium" { (p.slogan_flat()) }
                                div class="text-xs opacity-60" { "$" (p.price) }
                            }
                            div class="text-sm" {
                                @if p.printify_product_id.is_some() {
                                    span class="text-green-600" { "Synced" }
                                } @else {
                                    span class="text-amber-600" { "Pending" }
                                }
                            }
                        }
                    }
                }
            }

            div class="mt-8" {
                h2 class="font-semibold" { "Sync log" }
                div class="mt-2 h-80 overflow-y-auto rounded-md border bg-white/60 p-3 font-mono text-xs" {
                    template x-if="log.length === 0" {
                        p class="opacity-60" { "No activity yet. Run a sync to see step-by-step progress here." }
                    }
                    template x-for="(l, i) in log" ":key"="i" {
                        div ":class"="l.status === 'error' ? 'text-red-600' : (l.status === 'ok' ? 'text-green-700' : 'opacity-60')" {
                            span class="font-semibold" { "[" span x-text="l.slug" {} "] " }
                            span class="uppercase" x-text="l.name" {}
                            span { " - " } span x-text="l.detail" {}
                        }
                    }
                }
            }
        }
        (sync_script())
    };

    Ok(Html(shell("Printify Sync - RBE Admin", "Sync RBE products to Printify.", Nav::None, body).into_string()))
}

/// SSE endpoint: streams one event per sync step, then a `done` event.
#[derive(Deserialize)]
pub struct SyncQuery {
    shop: String,
    /// "pending" | "all"
    scope: String,
}

pub async fn printify_sync_stream(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(q): Query<SyncQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let products: Vec<Product> = match state
            .db()
            .query("SELECT * FROM product ORDER BY slug")
            .await
            .and_then(|mut r| r.take(0))
        {
            Ok(p) => p,
            Err(e) => {
                let _ = tx
                    .send(Ok(Event::default().event("error").data(e.to_string())))
                    .await;
                return;
            }
        };

        let printify = Printify::new(state.cfg(), state.http());
        let targets: Vec<Product> = products
            .into_iter()
            .filter(|p| q.scope == "all" || p.printify_product_id.is_none())
            .collect();

        let mut ok = 0;
        let mut failed = 0;
        for product in &targets {
            let (success, _action, steps) =
                printify.sync_one(state.db(), &q.shop, &product.slug).await;
            for s in steps {
                let payload = json!({ "slug": product.slug, "name": s.name, "status": s.status, "detail": s.detail });
                let _ = tx
                    .send(Ok(Event::default().event("step").data(payload.to_string())))
                    .await;
            }
            if success {
                ok += 1;
            } else {
                failed += 1;
            }
        }

        let summary = json!({ "ok": ok, "failed": failed, "total": targets.len() });
        let _ = tx
            .send(Ok(Event::default().event("done").data(summary.to_string())))
            .await;
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

fn sync_script() -> Markup {
    maud::PreEscaped(
        r#"<script>
function printifySync(){
  return {
    shopId: '',
    running: false,
    log: [],
    run(scope){
      if (!this.shopId || this.running) return;
      this.running = true; this.log = [];
      const es = new EventSource(`/admin/printify/sync?shop=${encodeURIComponent(this.shopId)}&scope=${scope}`);
      es.addEventListener('step', (e) => this.log.push(JSON.parse(e.data)));
      es.addEventListener('error', () => { this.log.push({ slug:'-', name:'error', status:'error', detail:'stream error' }); });
      es.addEventListener('done', () => { this.running = false; es.close(); });
    }
  };
}
</script>"#
            .to_string(),
    )
}
