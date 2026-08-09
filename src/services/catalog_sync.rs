use std::collections::{HashMap, HashSet};

use crate::db;
use crate::models::Product;
use crate::state::AppState;

use super::shopify::{Product as ShopifyProduct, Shopify};

pub async fn sync_shopify_catalog(state: &AppState) -> anyhow::Result<()> {
    let shopify = Shopify::new(state.cfg(), state.http());
    let remote = shopify.products(50).await?;
    let remote: Vec<ShopifyProduct> = remote
        .into_iter()
        .filter(|product| !is_placeholder_product(&product.handle))
        .collect();

    let existing: Vec<Product> = state
        .db()
        .query("SELECT * FROM product")
        .await?
        .take(0)?;

    let existing_by_handle: HashMap<String, Product> = existing
        .iter()
        .filter_map(|p| p.shopify_handle.as_ref().map(|h| (h.clone(), p.clone())))
        .collect();
    let existing_by_slug: HashMap<String, Product> = existing
        .iter()
        .map(|p| (p.slug.clone(), p.clone()))
        .collect();

    let mut active_slugs = HashSet::new();
    let mut active_handles = HashSet::new();

    for remote_product in remote {
        let curated = curated_product(&remote_product, &existing_by_handle, &existing_by_slug);
        active_handles.insert(curated.storefront_handle().to_string());
        active_slugs.insert(curated.slug.clone());
        db::upsert_product(state.db(), &curated).await?;
    }

    for product in existing {
        let handle = product.storefront_handle().to_string();
        if !active_slugs.contains(&product.slug) && !active_handles.contains(&handle) {
            db::delete_product(state.db(), &product.slug).await?;
        }
    }

    let _ = db::bump_version(state.db(), "shopify-catalog-sync").await;
    Ok(())
}

fn curated_product(
    remote: &ShopifyProduct,
    existing_by_handle: &HashMap<String, Product>,
    existing_by_slug: &HashMap<String, Product>,
) -> Product {
    let override_row = catalog_override(&remote.handle);
    let fallback_slug = override_row
        .map(|row| row.slug.to_string())
        .unwrap_or_else(|| derived_slug(remote));

    let existing = existing_by_handle
        .get(&remote.handle)
        .or_else(|| existing_by_slug.get(&fallback_slug));

    let slogan = override_row
        .map(|row| row.slogan.to_string())
        .or_else(|| existing.map(|row| row.slogan.clone()))
        .unwrap_or_else(|| derived_slogan(remote));

    let vibe = override_row
        .map(|row| row.vibe.to_string())
        .or_else(|| existing.map(|row| row.vibe.clone()))
        .unwrap_or_else(|| derived_vibe(remote));

    let tee_color = override_row
        .map(|row| row.tee_color.to_string())
        .or_else(|| existing.map(|row| row.tee_color.clone()))
        .unwrap_or_else(|| color_hex(remote).to_string());

    let ink_color = override_row
        .map(|row| row.ink_color.to_string())
        .or_else(|| existing.map(|row| row.ink_color.clone()))
        .unwrap_or_else(|| "#111111".to_string());

    let font_class = override_row
        .map(|row| row.font_class.to_string())
        .or_else(|| existing.map(|row| row.font_class.clone()))
        .unwrap_or_else(|| "font-display".to_string());

    let scale = override_row
        .map(|row| row.scale)
        .or_else(|| existing.map(|row| row.scale))
        .unwrap_or(1.0);

    let price = remote
        .price_range
        .min_variant_price
        .amount
        .parse::<f64>()
        .ok()
        .map(|v| v.round() as i64)
        .unwrap_or_else(|| existing.map(|row| row.price).unwrap_or(0));

    let image = remote
        .first_image()
        .map(|image| image.url.clone())
        .or_else(|| existing.map(|row| row.image.clone()))
        .unwrap_or_else(|| "/static/img/hero-tee.jpg".to_string());

    let description = if remote.description.trim().is_empty() {
        existing
            .map(|row| row.description.clone())
            .unwrap_or_else(|| "Printed on demand. Secure checkout via Shopify.".to_string())
    } else {
        remote.description.clone()
    };

    Product {
        slug: override_row
            .map(|row| row.slug.to_string())
            .unwrap_or_else(|| existing.map(|row| row.slug.clone()).unwrap_or(fallback_slug)),
        shopify_handle: Some(remote.handle.clone()),
        slogan,
        price,
        tee_color,
        ink_color,
        font_class,
        scale,
        description,
        vibe,
        image,
        printify_product_id: existing.and_then(|row| row.printify_product_id.clone()),
        printify_status: existing.and_then(|row| row.printify_status.clone()),
        printify_shop_id: existing.and_then(|row| row.printify_shop_id.clone()),
    }
}

fn is_placeholder_product(handle: &str) -> bool {
    matches!(
        handle,
        "snow-washed-oversized-cotton-t-shirt" | "unisex-seamless-cotton-t-shirt"
    )
}

fn derived_slug(remote: &ShopifyProduct) -> String {
    if let Some(quoted) = first_quoted_segment(&remote.title) {
        return slugify(quoted);
    }
    slugify(&remote.handle)
}

fn derived_slogan(remote: &ShopifyProduct) -> String {
    if let Some(quoted) = first_quoted_segment(&remote.title) {
        return quoted
            .split_whitespace()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|chunk| chunk.join(" "))
            .collect::<Vec<_>>()
            .join("\n");
    }

    remote
        .handle
        .split('-')
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|chunk| chunk.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn derived_vibe(remote: &ShopifyProduct) -> String {
    if remote.title.to_ascii_lowercase().contains("crop") {
        "Crop energy".to_string()
    } else if remote.title.to_ascii_lowercase().contains("oversized") {
        "Oversized".to_string()
    } else {
        "Shopify live".to_string()
    }
}

fn color_hex(remote: &ShopifyProduct) -> &'static str {
    let color = remote
        .options
        .iter()
        .find(|option| option.name.eq_ignore_ascii_case("Color"))
        .and_then(|option| option.values.first())
        .map(|value| value.as_str())
        .unwrap_or("White");

    match color.to_ascii_lowercase().as_str() {
        "black" => "#0a0a0a",
        "red" | "rose red" => "#c8102e",
        "athletic heather" | "grey marle" => "#d0d0d0",
        "pink" => "#ff2b8f",
        _ => "#ffffff",
    }
}

fn first_quoted_segment(title: &str) -> Option<&str> {
    let quote_chars = ['"', '“', '”', '„', '‟'];
    let start = title.find(quote_chars)?;
    let tail = &title[start + title[start..].chars().next()?.len_utf8()..];
    let end = tail.find(quote_chars)?;
    Some(tail[..end].trim())
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

#[derive(Clone, Copy)]
struct CatalogOverride {
    slug: &'static str,
    slogan: &'static str,
    vibe: &'static str,
    tee_color: &'static str,
    ink_color: &'static str,
    font_class: &'static str,
    scale: f64,
}

fn catalog_override(handle: &str) -> Option<CatalogOverride> {
    let row = match handle {
        "father-figure-crop-tee-womens-feminist-statement-top" => CatalogOverride {
            slug: "father-figure",
            slogan: "FATHER\nFIGURE",
            vibe: "Provider",
            tee_color: "#ffffff",
            ink_color: "#c8102e",
            font_class: "font-serif-display",
            scale: 1.0,
        },
        "oversized-tee-im-boring-baby-all-i-do-is-make-money-come-home-graphic" => {
            CatalogOverride {
                slug: "boring-baby",
                slogan: "I'm boring baby,\nall I do is\nmake money\n& come home.",
                vibe: "Grown",
                tee_color: "#ffffff",
                ink_color: "#e60023",
                font_class: "font-serif-display",
                scale: 0.65,
            }
        }
        "hot-girls-go-to-therapy-t-shirt-feminist-self-care-graphic-tee" => CatalogOverride {
            slug: "hot-girls-go-to-therapy",
            slogan: "Hot Girls\nGo To Therapy",
            vibe: "Soft power",
            tee_color: "#ffc7dd",
            ink_color: "#c1153f",
            font_class: "font-serif-display",
            scale: 1.0,
        },
        "slay-dhd" => CatalogOverride {
            slug: "slay-dhd",
            slogan: "SLAY-\nDHD",
            vibe: "Unbothered",
            tee_color: "#ffffff",
            ink_color: "#111111",
            font_class: "font-display",
            scale: 1.0,
        },
        "graphic-tee-the-hot-unmarried-aunty-funny-statement-t-shirt" => CatalogOverride {
            slug: "hot-unmarried-aunty",
            slogan: "THE HOT\nUNMARRIED\nAUNTY",
            vibe: "Chaos auntie",
            tee_color: "#ffffff",
            ink_color: "#c8102e",
            font_class: "font-display",
            scale: 0.9,
        },
        "graphic-tee-i-m-literally-just-a-girl-oversized-shirt" => CatalogOverride {
            slug: "im-literally-just-a-girl",
            slogan: "I'm literally\njust a girl",
            vibe: "Serve",
            tee_color: "#ff2b8f",
            ink_color: "#ffffff",
            font_class: "font-serif-display",
            scale: 1.0,
        },
        "crop-tee-body-so-tea-the-british-are-coming-funny-slogan-womens-crop-top" => {
            CatalogOverride {
                slug: "body-so-tea",
                slogan: "BODY SO TEA\nTHE BRITISH\nARE COMING",
                vibe: "Petty",
                tee_color: "#ffffff",
                ink_color: "#111111",
                font_class: "font-display",
                scale: 0.84,
            }
        }
        "graphic-tee-girls-just-wanna-have-funds-oversized-white-t-shirt" => CatalogOverride {
            slug: "girls-just-wanna-have-funds",
            slogan: "GIRLS JUST\nWANNA HAVE\nFUNDS",
            vibe: "Money",
            tee_color: "#ffffff",
            ink_color: "#111111",
            font_class: "font-display",
            scale: 0.92,
        },
        "future-milf-graphic-tee-minimal-white-oversized-t-shirt" => CatalogOverride {
            slug: "future-milf",
            slogan: "FUTURE\nMILF",
            vibe: "Soft menace",
            tee_color: "#ffffff",
            ink_color: "#111111",
            font_class: "font-display",
            scale: 1.0,
        },
        "graphic-tee-well-behaved-women-dont-make-history-oversized-statement-t-shirt" => {
            CatalogOverride {
                slug: "well-behaved-women",
                slogan: "WELL BEHAVED\nWOMEN DON'T\nMAKE HISTORY",
                vibe: "History",
                tee_color: "#ffffff",
                ink_color: "#111111",
                font_class: "font-display",
                scale: 0.8,
            }
        }
        "bad-bitch-club-t-shirt-feminist-slogan-tee-minimal-red-text" => CatalogOverride {
            slug: "bad-b-club",
            slogan: "BAD B\nCLUB",
            vibe: "Members only",
            tee_color: "#ffffff",
            ink_color: "#ff2b8f",
            font_class: "font-display",
            scale: 1.2,
        },
        _ => return None,
    };
    Some(row)
}
