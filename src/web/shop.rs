//! Storefront. The catalog is **database-driven**: the grid and product pages
//! render from SurrealDB (the seeded RBE designs), so the shop is never empty.
//! Checkout **checks Shopify** — for each item we look the product up in Shopify
//! by handle and use its live variant, so checkout activates automatically once
//! a product is published there (via the Printify sync or Shopify admin).

use axum::Json;
use axum::extract::{Path, State};
use axum::response::Html;
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::models::{Product, SIZES};
use crate::services::shopify::Shopify;
use crate::state::AppState;

use super::layout::{Nav, shell};
use super::tee_mockup;

pub async fn shop_index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;

    let body = html! {
        div class="mx-auto max-w-7xl px-4 py-16 md:px-8" {
            div class="mb-10" {
                div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "The full drop" }
                h1 class="mt-2 font-display text-6xl md:text-8xl" { "SHOP" }
                p class="mt-3 max-w-xl font-serif-display text-xl opacity-70" {
                    (products.len()) " " (if products.len() == 1 { "slogan" } else { "slogans" }) ". One energy."
                }
            }
            div class="grid grid-cols-2 gap-4 md:grid-cols-3 md:gap-6 lg:grid-cols-4" {
                @for p in &products { (product_card(p)) }
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
    html! {
        a href=(format!("/shop/{}", p.slug)) class="group block" {
            div class="overflow-hidden rounded-md bg-white p-3 transition group-hover:-translate-y-1 group-hover:shadow-xl group-hover:shadow-[color:var(--hot)]/20" {
                (tee_mockup(&p.image, &p.slogan_flat(), &p.tee_color))
            }
            div class="mt-3 flex items-center justify-between" {
                div class="text-sm font-semibold uppercase" { (p.slogan_first_line()) }
                div class="text-sm" { "$" (p.price) }
            }
            div class="text-xs uppercase tracking-widest opacity-60" { (p.vibe) }
        }
    }
}

pub async fn product_detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let product: Option<Product> = state
        .db()
        .query("SELECT * FROM type::thing('product', $slug)")
        .bind(("slug", slug))
        .await?
        .take(0)?;
    let product = product.ok_or(AppError::NotFound)?;

    let body = html! {
        div class="bg-[color:var(--cream)]"
            x-data=(format!("productPage({})", product_json(&product))) {
            div class="mx-auto max-w-7xl px-4 pb-16 pt-8 md:px-8" {
                a href="/shop" class="text-xs uppercase tracking-widest opacity-60 hover:opacity-100" { "← All tees" }
                div class="mt-6 grid gap-10 md:grid-cols-2" {
                    div class="rounded-lg bg-white p-6 shadow-xl shadow-[color:var(--hot)]/20" {
                        (tee_mockup(&product.image, &product.slogan_flat(), &product.tee_color))
                    }
                    div class="flex flex-col" {
                        div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "RBE Drop · " (product.vibe) }
                        h1 class="mt-2 font-display text-5xl leading-none md:text-6xl" { (product.slogan_flat()) }
                        div class="mt-6 text-2xl font-semibold" { "$" (product.price) }
                        p class="mt-4 max-w-md font-serif-display text-xl opacity-80" { (product.description) }

                        div class="mt-8" {
                            div class="mb-2 text-xs uppercase tracking-widest opacity-70" { "Size" }
                            div class="flex flex-wrap gap-2" {
                                @for s in SIZES {
                                    button
                                        "@click"=(format!("size = '{s}'"))
                                        ":class"=(format!("size === '{s}' ? 'border-[color:var(--hot)] bg-[color:var(--hot)] text-white' : 'border-ink/20 hover:border-ink'"))
                                        class="min-w-11 h-11 rounded-full border px-4 text-sm font-semibold transition" {
                                        (s)
                                    }
                                }
                            }
                        }

                        div class="mt-8 flex flex-col gap-3 sm:flex-row" {
                            button "@click"="addToBag()"
                                class="inline-flex items-center justify-center gap-2 rounded-full bg-ink px-8 py-4 font-display text-lg uppercase tracking-widest text-cream transition hover:bg-[color:var(--hot)]"
                                x-text=(format!("added ? 'Added ✓' : 'Add to bag — ${}'", product.price)) {}
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
        &format!("{} — RBE", product.slogan_flat()),
        &product.description,
        Nav::Shop,
        body,
    ).into_string()))
}

/// JSON blob the product-page Alpine component needs for add-to-bag.
fn product_json(p: &Product) -> String {
    json!({
        "slug": p.slug,
        "title": p.slogan_flat(),
        "image": p.image,
        "price": p.price,
    })
    .to_string()
}

/// POST /api/checkout — body `{ items: [{ slug, size, qty }] }`.
/// Looks each product up in Shopify by handle and builds a Shopify cart, so
/// checkout only works for products actually published to Shopify.
#[derive(Deserialize)]
pub struct CheckoutItem {
    slug: String,
    #[serde(default)]
    size: Option<String>,
    qty: i32,
}

#[derive(Deserialize)]
pub struct CheckoutReq {
    items: Vec<CheckoutItem>,
}

pub async fn checkout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CheckoutReq>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    use axum::http::StatusCode;
    let err = |code: StatusCode, msg: String| (code, Json(json!({ "error": msg })));

    // Checkout is for signed-in shoppers: it gives them order history + tracking
    // under their account, and lets us tie the Shopify order back to a customer.
    if crate::auth::customer_email(&headers, state.cfg()).is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Please sign in to check out.",
                "login_required": true
            })),
        );
    }

    if req.items.is_empty() {
        return err(StatusCode::BAD_REQUEST, "Your bag is empty.".into());
    }
    let shopify = Shopify::new(state.cfg(), state.http());

    let mut lines: Vec<(String, i32)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for item in &req.items {
        let local: Option<Product> = match state
            .db()
            .query("SELECT * FROM type::thing('product', $slug)")
            .bind(("slug", item.slug.clone()))
            .await
        {
            Ok(mut r) => r.take(0).ok().flatten(),
            Err(_) => None,
        };
        let handle = local
            .as_ref()
            .map(|p| p.storefront_handle().to_string())
            .unwrap_or_else(|| item.slug.clone());
        let found = match shopify.admin_product_by_handle(&handle).await {
            Ok(v) => v,
            Err(e) => return err(StatusCode::BAD_GATEWAY, format!("Shopify error: {e}")),
        };
        match found {
            Some(sp) => {
                // Prefer the variant matching the chosen size; else first variant.
                let variant = item
                    .size
                    .as_ref()
                    .and_then(|sz| {
                        sp.variants.0.iter().find(|v| {
                            v.selected_options
                                .iter()
                                .any(|o| o.name.eq_ignore_ascii_case("size") && &o.value == sz)
                        })
                    })
                    .or_else(|| sp.variants.0.first());
                match variant {
                    Some(v) => lines.push((v.legacy_resource_id.clone(), item.qty)),
                    None => missing.push(item.slug.clone()),
                }
            }
            None => missing.push(item.slug.clone()),
        }
    }

    if !missing.is_empty() {
        return err(
            StatusCode::CONFLICT,
            format!(
                "Not live for checkout yet: {}. These products need to be published to Shopify's Online Store before checkout can see them.",
                missing.join(", ")
            ),
        );
    }

    match shopify.cart_permalink(&lines) {
        Ok(url) => (StatusCode::OK, Json(json!({ "url": url }))),
        Err(e) => err(StatusCode::BAD_GATEWAY, format!("Shopify checkout error: {e}")),
    }
}

fn product_page_script() -> Markup {
    PreEscaped(
        r#"<script>
function productPage(product){
  return {
    product,
    size: 'M',
    added: false,
    addToBag(){
      this.$store.cart.add({
        id: this.product.slug + ':' + this.size,
        slug: this.product.slug,
        title: this.product.title,
        image: this.product.image,
        price: this.product.price,
        size: this.size,
        qty: 1,
      });
      this.added = true; setTimeout(() => this.added = false, 1500);
    }
  };
}
</script>"#.to_string(),
    )
}

/// Live shop-cache invalidation (SurrealDB event bus → SSE → reload).
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
