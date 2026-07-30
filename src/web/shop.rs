//! Storefront: shop grid, product detail, and the checkout hand-off to Shopify.
//! Ported from the reference `src/routes/shop.tsx` and `shop.$slug.tsx`.

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Html;
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::services::shopify::{Product, Shopify, format_money};
use crate::state::AppState;

use super::layout::{Nav, shell};

pub async fn shop_index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let shopify = Shopify::new(state.cfg(), state.http());
    // The storefront depends on Shopify; if it's unreachable we still render a
    // graceful empty state rather than erroring the whole page.
    let products = shopify.products(50).await.unwrap_or_default();

    let body = html! {
        div class="mx-auto max-w-7xl px-4 py-16 md:px-8" {
            div class="mb-10" {
                div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "The full drop" }
                h1 class="mt-2 font-display text-6xl md:text-8xl" { "SHOP" }
                p class="mt-3 max-w-xl font-serif-display text-xl opacity-70" {
                    @if products.is_empty() { "No products yet." }
                    @else { (products.len()) " " (if products.len() == 1 { "slogan" } else { "slogans" }) ". One energy." }
                }
            }
            @if products.is_empty() {
                div class="rounded-lg border border-dashed border-ink/20 bg-white/50 p-12 text-center" {
                    div class="font-display text-3xl" { "No products found" }
                    p class="mt-3 mx-auto max-w-md text-sm opacity-70" {
                        "The Shopify store has no products yet, or the storefront is unreachable. Sync from Printify at "
                        a href="/admin/printify" class="underline" { "/admin/printify" } "."
                    }
                }
            } @else {
                div class="grid grid-cols-2 gap-4 md:grid-cols-3 md:gap-6 lg:grid-cols-4" {
                    @for p in &products { (product_card(p)) }
                }
            }
        }
        (cache_bust_script())
    };

    Ok(Html(shell(
        "Shop — RBE Slogan Tees",
        "Shop the RBE tee drop. Loud slogans, soft cotton, printed on demand.",
        Nav::Shop,
        body,
    ).into_string()))
}

fn product_card(p: &Product) -> Markup {
    let img = p.first_image();
    let price = &p.price_range.min_variant_price;
    html! {
        a href=(format!("/shop/{}", p.handle)) class="group block" {
            div class="aspect-square overflow-hidden rounded-md bg-white transition group-hover:-translate-y-1 group-hover:shadow-xl group-hover:shadow-[color:var(--hot)]/20" {
                @if let Some(img) = img {
                    img src=(img.url) alt=(img.alt_text.clone().unwrap_or_else(|| p.title.clone()))
                        class="h-full w-full object-cover" loading="lazy";
                } @else {
                    div class="flex h-full w-full items-center justify-center text-xs opacity-40" { "No image" }
                }
            }
            div class="mt-3 flex items-center justify-between" {
                div class="text-sm font-semibold uppercase" { (p.title) }
                div class="text-sm" { (format_money(&price.amount, &price.currency_code)) }
            }
        }
    }
}

pub async fn product_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let shopify = Shopify::new(state.cfg(), state.http());
    let product = shopify.product_by_handle(&slug).await?.ok_or(AppError::NotFound)?;

    let img = product.first_image();
    let img_url = img.map(|i| i.url.clone());
    let price = &product.price_range.min_variant_price;

    // Serialize the fields the Alpine component needs for variant selection.
    let data = json!({
        "title": product.title,
        "image": img_url,
        "options": product.options.iter().map(|o| json!({ "name": o.name, "values": o.values })).collect::<Vec<_>>(),
        "variants": product.variants.0.iter().map(|v| json!({
            "id": v.id,
            "availableForSale": v.available_for_sale,
            "price": { "amount": v.price.amount, "currencyCode": v.price.currency_code },
            "selectedOptions": v.selected_options.iter().map(|so| json!({ "name": so.name, "value": so.value })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });

    let body = html! {
        div class="bg-[color:var(--cream)]" x-data="productPage()" {
            script { (PreEscaped(format!("window.__PRODUCT__ = {};", data))) }
            div class="mx-auto max-w-7xl px-4 pb-16 pt-8 md:px-8" {
                a href="/shop" class="text-xs uppercase tracking-widest opacity-60 hover:opacity-100" { "← All tees" }
                div class="mt-6 grid gap-10 md:grid-cols-2" {
                    div class="rounded-lg bg-white p-6 shadow-xl shadow-[color:var(--hot)]/20" {
                        @if let Some(url) = &img_url {
                            img src=(url) alt=(product.title) class="mx-auto aspect-square w-full rounded-md object-cover";
                        } @else {
                            div class="flex aspect-square items-center justify-center text-sm opacity-40" { "No image" }
                        }
                    }
                    div class="flex flex-col" {
                        div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "RBE Drop" }
                        h1 class="mt-2 font-display text-5xl leading-none md:text-6xl" { (product.title) }
                        div class="mt-6 text-2xl font-semibold"
                            x-text="'$' + parseFloat(activePrice.amount).toFixed(2)" {
                            (format_money(&price.amount, &price.currency_code))
                        }
                        @if !product.description.is_empty() {
                            p class="mt-4 max-w-md font-serif-display text-xl opacity-80" { (product.description) }
                        }

                        template x-for="opt in data.options" ":key"="opt.name" {
                            div class="mt-8" {
                                div class="mb-2 text-xs uppercase tracking-widest opacity-70" x-text="opt.name" {}
                                div class="flex flex-wrap gap-2" {
                                    template x-for="v in opt.values" ":key"="v" {
                                        button
                                            "@click"="selected[opt.name] = v"
                                            ":class"="selected[opt.name] === v ? 'border-[color:var(--hot)] bg-[color:var(--hot)] text-white' : 'border-ink/20 hover:border-ink'"
                                            class="min-w-11 h-11 rounded-full border px-4 text-sm font-semibold transition"
                                            x-text="v" {}
                                    }
                                }
                            }
                        }

                        div class="mt-8 flex flex-col gap-3 sm:flex-row" {
                            button "@click"="addToBag()"
                                ":disabled"="!activeVariant || !activeVariant.availableForSale"
                                class="inline-flex items-center justify-center gap-2 rounded-full bg-ink px-8 py-4 font-display text-lg uppercase tracking-widest text-cream transition hover:bg-[color:var(--hot)] disabled:opacity-40"
                                x-text="added ? 'Added ✓' : (activeVariant && !activeVariant.availableForSale ? 'Sold out' : 'Add to bag — $' + parseFloat(activePrice.amount).toFixed(2))" {}
                        }

                        ul class="mt-8 space-y-2 text-sm opacity-80" {
                            li { "· Printed on demand" }
                            li { "· Ships in 3–5 business days" }
                            li { "· Secure checkout via Shopify" }
                        }
                    }
                }
            }
        }
        (product_page_script())
    };

    Ok(Html(shell(
        &format!("{} — RBE", product.title),
        if product.description.is_empty() { &product.title } else { &product.description },
        Nav::Shop,
        body,
    ).into_string()))
}

/// POST /api/checkout — body `{ items: [{ variantId, qty }] }` → Shopify cart URL.
#[derive(Deserialize)]
pub struct CheckoutItem {
    #[serde(rename = "variantId")]
    variant_id: String,
    qty: i32,
}

#[derive(Deserialize)]
pub struct CheckoutReq {
    items: Vec<CheckoutItem>,
}

pub async fn checkout(
    State(state): State<AppState>,
    Json(req): Json<CheckoutReq>,
) -> AppResult<Json<serde_json::Value>> {
    if req.items.is_empty() {
        return Err(AppError::BadRequest("cart is empty".into()));
    }
    let shopify = Shopify::new(state.cfg(), state.http());
    let lines: Vec<(String, i32)> = req.items.into_iter().map(|i| (i.variant_id, i.qty)).collect();
    let url = shopify.create_cart(&lines).await?;
    Ok(Json(json!({ "url": url })))
}

fn product_page_script() -> Markup {
    PreEscaped(
        r#"<script>
function productPage(){
  return {
    data: window.__PRODUCT__,
    selected: {},
    added: false,
    init(){ for (const o of this.data.options) this.selected[o.name] = o.values[0]; },
    get activeVariant(){
      return this.data.variants.find(v => v.selectedOptions.every(so => this.selected[so.name] === so.value)) || this.data.variants[0];
    },
    get activePrice(){ const v = this.activeVariant; return v ? v.price : { amount: '0', currencyCode: 'USD' }; },
    addToBag(){
      const v = this.activeVariant; if (!v) return;
      const sizeOpt = this.data.options.find(o => o.name.toLowerCase() === 'size');
      this.$store.cart.add({
        variantId: v.id, title: this.data.title, image: this.data.image,
        price: parseFloat(v.price.amount), currency: v.price.currencyCode,
        size: sizeOpt ? this.selected[sizeOpt.name] : null, qty: 1
      });
      this.added = true; setTimeout(() => this.added = false, 1500);
    }
  };
}
</script>"#.to_string(),
    )
}

/// Live shop-cache invalidation: subscribe to server-sent events and reload the
/// grid when a webhook bumps the version (SurrealDB LIVE → SSE).
fn cache_bust_script() -> Markup {
    PreEscaped(
        r#"<script>
(function(){
  try {
    const es = new EventSource('/events');
    let first = true;
    es.addEventListener('cache', () => { if (first) { first = false; return; } location.reload(); });
  } catch (e) {}
})();
</script>"#.to_string(),
    )
}
