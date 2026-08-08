//! Admin control panel: website management, Shopify visibility, Printify sync
//! (legacy for now), and team access.

use axum::Form;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::response::sse::{Event, KeepAlive, Sse};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::StaffUser;
use crate::db;
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
    staff_accounts: usize,
    customer_accounts: usize,
}

pub async fn dashboard(
    user: StaffUser,
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
                div class="flex flex-wrap items-center gap-3 sm:justify-end" {
                    a href="/dashboard" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- Dashboard" }
                    a href="/auth/logout" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "Sign out" }
                }
            }

            div class="mt-10 grid gap-4 sm:grid-cols-2 xl:grid-cols-5" {
                (stat("Catalog items", &stats.products.to_string()))
                (stat("Visible in Shopify", &stats.live_shopify_products.to_string()))
                (stat("Orders", &stats.orders.to_string()))
                (stat("Journal posts", &stats.posts.to_string()))
                (stat("Published posts", &stats.published_posts.to_string()))
            }

            div class="mt-4 grid gap-4 sm:grid-cols-2" {
                (stat("Staff accounts", &stats.staff_accounts.to_string()))
                (stat("Customer accounts", &stats.customer_accounts.to_string()))
            }

            div class="mt-10 grid gap-4 lg:grid-cols-3" {
                (admin_card(
                    "Product manager",
                    "Add, edit, remove, and connect catalog items to the right Shopify handles.",
                    "/admin/products",
                ))
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
                @if user.is_admin() {
                    (admin_card(
                        "Users & access",
                        "Create owner accounts, reset passwords, and manage shopper logins.",
                        "/admin/team",
                    ))
                }
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
    let staff_rows: Vec<CountRow> = state
        .db()
        .query("SELECT count() AS count FROM staff GROUP ALL")
        .await?
        .take(0)?;
    let customer_rows: Vec<CountRow> = state
        .db()
        .query("SELECT count() AS count FROM customer GROUP ALL")
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
        staff_accounts: staff_rows.first().map(|r| r.count).unwrap_or(0),
        customer_accounts: customer_rows.first().map(|r| r.count).unwrap_or(0),
    })
}

struct ProductVisibility {
    product: Product,
    shopify: ShopifyProduct,
}

pub async fn shopify_page(
    _user: StaffUser,
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

pub async fn products_page(
    _user: StaffUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;

    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-16" {
            div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Catalog manager" }
                    h1 class="mt-2 font-display text-5xl" { "Products" }
                    p class="mt-2 max-w-2xl text-sm opacity-70" {
                        "This is your site catalog. The slug controls the page URL on this website, and the Shopify handle tells checkout which Shopify product to open."
                    }
                }
                div class="flex flex-wrap items-center gap-3 sm:justify-end" {
                    a href="/admin" class="inline-flex items-center text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- Admin" }
                    a href="/admin/products/new" class="inline-flex items-center justify-center rounded-full bg-[color:var(--hot)] px-5 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "New product" }
                }
            }

            div class="mt-8 overflow-hidden rounded-xl border border-ink/10 bg-white" {
                @if products.is_empty() {
                    div class="px-6 py-10 text-sm opacity-60" { "No products yet. Add your first one." }
                } @else {
                    ul class="divide-y" {
                        @for product in &products {
                            li class="flex flex-col gap-4 px-6 py-5 md:flex-row md:items-start md:justify-between" {
                                div class="min-w-0 flex-1" {
                                    div class="font-medium" { (product.slogan_flat()) }
                                    div class="mt-1 text-xs uppercase tracking-widest opacity-60" { "Slug: " (product.slug.clone()) }
                                    div class="mt-1 text-[11px] opacity-50 break-all" { "Shopify handle: " (product.storefront_handle()) }
                                    div class="mt-2 flex flex-wrap gap-2 text-[11px] uppercase tracking-widest" {
                                        span class="rounded-full bg-black/5 px-2 py-1" { "$" (product.price) }
                                        span class="rounded-full bg-black/5 px-2 py-1" { (product.vibe.clone()) }
                                        @if product.shopify_handle.is_some() {
                                            span class="rounded-full bg-[color:color-mix(in_oklab,var(--hot)_12%,transparent)] px-2 py-1 text-[color:var(--hot)]" { "Mapped" }
                                        } @else {
                                            span class="rounded-full bg-amber-100 px-2 py-1 text-amber-800" { "Using slug as handle" }
                                        }
                                    }
                                }
                                a href=(format!("/admin/products/{}/edit", product.slug)) class="inline-flex shrink-0 items-center justify-center self-start rounded-full border border-ink/20 px-4 py-2 text-sm font-semibold uppercase tracking-widest hover:border-[color:var(--hot)] hover:text-[color:var(--hot)]" { "Edit" }
                            }
                        }
                    }
                }
            }
        }
    };

    Ok(Html(shell(
        "Products - RBE Admin",
        "Manage storefront products and Shopify handle mappings.",
        Nav::None,
        body,
    ).into_string()))
}

pub async fn product_new(_user: StaffUser) -> Html<String> {
    Html(product_editor(None, None).into_string())
}

pub async fn product_edit(
    _user: StaffUser,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let product: Option<Product> = state
        .db()
        .query("SELECT * FROM type::thing('product', $slug)")
        .bind(("slug", slug))
        .await?
        .take(0)?;
    Ok(Html(product_editor(product.as_ref(), None).into_string()))
}

#[derive(Deserialize, Clone)]
pub struct ProductForm {
    #[serde(default)]
    original_slug: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    shopify_handle: String,
    #[serde(default)]
    slogan: String,
    #[serde(default)]
    price: String,
    #[serde(default)]
    tee_color: String,
    #[serde(default)]
    ink_color: String,
    #[serde(default)]
    font_class: String,
    #[serde(default)]
    scale: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    vibe: String,
    #[serde(default)]
    image: String,
}

pub async fn product_save(
    _user: StaffUser,
    State(state): State<AppState>,
    Form(f): Form<ProductForm>,
) -> Response {
    let title_hint = f.slogan.lines().next().unwrap_or_default();
    let slug = if f.slug.trim().is_empty() {
        slugify(title_hint)
    } else {
        slugify(&f.slug)
    };
    if slug.is_empty() {
        return product_editor_error("Add a slogan or slug so the product has a page URL.", &f)
            .into_response();
    }

    let price = match f.price.trim().parse::<i64>() {
        Ok(v) if v >= 0 => v,
        _ => {
            return product_editor_error("Price must be a whole number like 40.", &f)
                .into_response();
        }
    };

    let scale = if f.scale.trim().is_empty() {
        1.0
    } else {
        match f.scale.trim().parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            _ => {
                return product_editor_error("Scale must be a positive number like 1 or 0.95.", &f)
                    .into_response();
            }
        }
    };

    let original_slug = f.original_slug.trim();
    let existing: Option<Product> = if original_slug.is_empty() {
        None
    } else {
        state
            .db()
            .query("SELECT * FROM type::thing('product', $slug)")
            .bind(("slug", original_slug.to_string()))
            .await
            .ok()
            .and_then(|mut r| r.take(0).ok().flatten())
    };

    let product = Product {
        slug: slug.clone(),
        shopify_handle: (!f.shopify_handle.trim().is_empty())
            .then(|| f.shopify_handle.trim().to_string()),
        slogan: f.slogan.trim().to_string(),
        price,
        tee_color: f.tee_color.trim().to_string(),
        ink_color: f.ink_color.trim().to_string(),
        font_class: if f.font_class.trim().is_empty() {
            "font-display".to_string()
        } else {
            f.font_class.trim().to_string()
        },
        scale,
        description: f.description.trim().to_string(),
        vibe: f.vibe.trim().to_string(),
        image: f.image.trim().to_string(),
        printify_product_id: existing
            .as_ref()
            .and_then(|p| p.printify_product_id.clone()),
        printify_status: existing.as_ref().and_then(|p| p.printify_status.clone()),
        printify_shop_id: existing.as_ref().and_then(|p| p.printify_shop_id.clone()),
    };

    if product.slogan.trim().is_empty()
        || product.description.trim().is_empty()
        || product.image.trim().is_empty()
        || product.tee_color.trim().is_empty()
        || product.ink_color.trim().is_empty()
    {
        return product_editor_error(
            "Fill in the slogan, description, image, tee color, and ink color before saving.",
            &f,
        )
        .into_response();
    }

    let save = db::upsert_product(state.db(), &product).await;
    if let Err(_) = save {
        return product_editor_error(
            "Could not save this product. The slug may already belong to another item.",
            &f,
        )
        .into_response();
    }

    if !original_slug.is_empty() && original_slug != slug {
        let _ = db::delete_product(state.db(), original_slug).await;
    }

    Redirect::to("/admin/products").into_response()
}

pub async fn product_delete(
    _user: StaffUser,
    State(state): State<AppState>,
    Form(f): Form<ProductDeleteForm>,
) -> Response {
    let slug = f.slug.trim();
    if slug.is_empty() {
        return Redirect::to("/admin/products").into_response();
    }
    let _ = db::delete_product(state.db(), slug).await;
    Redirect::to("/admin/products").into_response()
}

#[derive(Deserialize)]
pub struct ProductDeleteForm {
    slug: String,
}

fn product_editor_error(msg: &str, f: &ProductForm) -> Html<String> {
    let product = Product {
        slug: f.slug.clone(),
        shopify_handle: (!f.shopify_handle.trim().is_empty()).then(|| f.shopify_handle.clone()),
        slogan: f.slogan.clone(),
        price: f.price.trim().parse().unwrap_or_default(),
        tee_color: f.tee_color.clone(),
        ink_color: f.ink_color.clone(),
        font_class: if f.font_class.trim().is_empty() {
            "font-display".to_string()
        } else {
            f.font_class.clone()
        },
        scale: f.scale.trim().parse().unwrap_or(1.0),
        description: f.description.clone(),
        vibe: f.vibe.clone(),
        image: f.image.clone(),
        printify_product_id: None,
        printify_status: None,
        printify_shop_id: None,
    };
    Html(product_editor(Some(&product), Some(msg)).into_string())
}

fn product_editor(product: Option<&Product>, error: Option<&str>) -> Markup {
    let field = "mt-1 w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
    let original_slug = product.map(|p| p.slug.clone()).unwrap_or_default();
    let slug = product.map(|p| p.slug.clone()).unwrap_or_default();
    let shopify_handle = product
        .and_then(|p| p.shopify_handle.clone())
        .unwrap_or_default();
    let slogan = product.map(|p| p.slogan.clone()).unwrap_or_default();
    let price = product.map(|p| p.price.to_string()).unwrap_or_default();
    let tee_color = product.map(|p| p.tee_color.clone()).unwrap_or_default();
    let ink_color = product.map(|p| p.ink_color.clone()).unwrap_or_default();
    let font_class = product
        .map(|p| p.font_class.clone())
        .unwrap_or_else(|| "font-display".to_string());
    let scale = product
        .map(|p| p.scale.to_string())
        .unwrap_or_else(|| "1".to_string());
    let description = product.map(|p| p.description.clone()).unwrap_or_default();
    let vibe = product.map(|p| p.vibe.clone()).unwrap_or_default();
    let image = product.map(|p| p.image.clone()).unwrap_or_default();
    let is_edit = product.is_some();

    let content = html! {
        div class="mx-auto max-w-3xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-4xl" {
                    @if is_edit { "Edit product" } @else { "New product" }
                }
                a href="/admin/products" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- All products" }
            }

            div class="mt-4 rounded-xl border border-ink/10 bg-white p-5 text-sm opacity-80" {
                p { "Slug = this website's page URL, like /shop/all-sugar-no-daddy." }
                p class="mt-2" { "Shopify handle = the Shopify product checkout should open for this item. Leave it blank if Shopify uses the same value as the slug." }
            }

            @if let Some(err) = error {
                p class="mt-4 text-sm text-red-500" { (err) }
            }

            @if is_edit {
                form method="post" action="/admin/products/delete" class="mt-6" {
                    input type="hidden" name="slug" value=(original_slug.clone());
                    button type="submit" class="rounded-full border border-red-300 px-5 py-2 text-sm font-semibold uppercase tracking-widest text-red-700 hover:bg-red-50" { "Delete product" }
                }
            }

            form method="post" action="/admin/products" class="mt-6 space-y-4" {
                input type="hidden" name="original_slug" value=(original_slug);

                div class="grid gap-4 md:grid-cols-2" {
                    div {
                        label class="text-sm font-medium" { "Slug" }
                        input name="slug" value=(slug) placeholder="auto from slogan" class=(field);
                    }
                    div {
                        label class="text-sm font-medium" { "Shopify handle" }
                        input name="shopify_handle" value=(shopify_handle) placeholder="usually matches Shopify product handle" class=(field);
                    }
                }

                div {
                    label class="text-sm font-medium" { "Slogan" }
                    textarea name="slogan" rows="3" class=(field) { (slogan) }
                }

                div class="grid gap-4 md:grid-cols-2" {
                    div {
                        label class="text-sm font-medium" { "Price" }
                        input name="price" value=(price) placeholder="40" class=(field);
                    }
                    div {
                        label class="text-sm font-medium" { "Vibe / collection" }
                        input name="vibe" value=(vibe) placeholder="Self-made" class=(field);
                    }
                }

                div class="grid gap-4 md:grid-cols-2" {
                    div {
                        label class="text-sm font-medium" { "Tee color" }
                        input name="tee_color" value=(tee_color) placeholder="#ffffff" class=(field);
                    }
                    div {
                        label class="text-sm font-medium" { "Ink color" }
                        input name="ink_color" value=(ink_color) placeholder="#ff005c" class=(field);
                    }
                }

                div class="grid gap-4 md:grid-cols-2" {
                    div {
                        label class="text-sm font-medium" { "Font class" }
                        input name="font_class" value=(font_class) placeholder="font-display" class=(field);
                    }
                    div {
                        label class="text-sm font-medium" { "Scale" }
                        input name="scale" value=(scale) placeholder="1" class=(field);
                    }
                }

                div {
                    label class="text-sm font-medium" { "Image path or URL" }
                    input name="image" value=(image) placeholder="/static/img/all-sugar-no-daddy.png" class=(field);
                }

                div {
                    label class="text-sm font-medium" { "Description" }
                    textarea name="description" rows="4" class=(field) { (description) }
                }

                div class="flex flex-wrap items-center justify-between gap-3 pt-2" {
                    @if !is_edit {
                        span class="text-xs uppercase tracking-widest opacity-50" { "New products appear in the site catalog immediately after save." }
                    }

                    button type="submit" class="rounded-full bg-[color:var(--hot)] px-6 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "Save product" }
                }
            }
        }
    };

    shell(
        "Product editor - RBE Admin",
        "Manage RBE product metadata and Shopify handle mappings.",
        Nav::None,
        content,
    )
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub async fn printify_page(
    _user: StaffUser,
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
    _user: StaffUser,
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
