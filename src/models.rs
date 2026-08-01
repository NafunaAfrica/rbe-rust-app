//! Domain models. These mirror the tables defined in `db.rs`.

use serde::{Deserialize, Serialize};

/// Internal product/design metadata. The customer-facing catalog & checkout
/// live in Shopify; this table holds the design definitions (colors, slogan,
/// fonts) and the Printify point-of-demand sync state.
///
/// Note: SurrealDB returns an `id` field on every record; we deliberately omit
/// it here and select by `slug`, so deserialization simply ignores it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub slug: String,
    #[serde(default)]
    pub shopify_handle: Option<String>,
    pub slogan: String,
    pub price: i64,
    pub tee_color: String,
    pub ink_color: String,
    /// "font-display" | "font-serif-display"
    pub font_class: String,
    #[serde(default = "one")]
    pub scale: f64,
    pub description: String,
    pub vibe: String,
    /// Path to the local mockup image, served under `/static/img/...`.
    pub image: String,

    // --- Printify sync state ---
    #[serde(default)]
    pub printify_product_id: Option<String>,
    #[serde(default)]
    pub printify_status: Option<String>,
    #[serde(default)]
    pub printify_shop_id: Option<String>,
}

fn one() -> f64 {
    1.0
}

/// Available sizes for every tee (the storefront is size-based; Shopify holds a
/// variant per size once a product is published there).
pub const SIZES: [&str; 6] = ["XS", "S", "M", "L", "XL", "2XL"];

/// A staff account: `admin` (Nafuna / full control) or `owner` (the store
/// owner's business view). Passwords are argon2 hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staff {
    pub email: String,
    pub password_hash: String,
    /// "admin" | "owner"
    pub role: String,
}

/// A shopper account. Order history is joined from ingested orders by email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub email: String,
    pub password_hash: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// A journal/blog post. Body is authored in Markdown. Timestamps are stored as
/// RFC3339 strings (set from Rust) to keep deserialization simple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub body_md: String,
    /// "draft" | "published"
    pub status: String,
    #[serde(default)]
    pub author: Option<String>,
    /// RFC3339 timestamp string, or None while a draft.
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// One line of an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLine {
    pub title: String,
    #[serde(default)]
    pub quantity: i64,
    #[serde(default)]
    pub price: Option<String>,
}

/// A customer order, ingested from Shopify webhooks. Fulfilment status and
/// tracking are updated as Shopify (via Printify's native integration) reports
/// them. Timestamps kept as strings for simple deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub shopify_order_id: String,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub total: String,
    #[serde(default)]
    pub financial_status: Option<String>,
    #[serde(default)]
    pub fulfillment_status: Option<String>,
    #[serde(default)]
    pub line_items: Vec<OrderLine>,
    #[serde(default)]
    pub tracking_url: Option<String>,
    #[serde(default)]
    pub tracking_number: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl Order {
    /// Human fulfilment label.
    pub fn fulfilment_label(&self) -> &str {
        match self.fulfillment_status.as_deref() {
            Some("fulfilled") => "Shipped",
            Some("partial") => "Partly shipped",
            Some(other) if !other.is_empty() => other,
            _ => "Unfulfilled",
        }
    }
    pub fn created_date(&self) -> &str {
        self.created_at
            .as_deref()
            .map(|s| s.get(0..10).unwrap_or(s))
            .unwrap_or("—")
    }
}

impl Post {
    pub fn is_published(&self) -> bool {
        self.status == "published"
    }
    /// Date portion (YYYY-MM-DD) of published_at for display.
    pub fn published_date(&self) -> &str {
        self.published_at
            .as_deref()
            .map(|s| s.get(0..10).unwrap_or(s))
            .unwrap_or("Draft")
    }
}

impl Product {
    pub fn storefront_handle(&self) -> &str {
        self.shopify_handle.as_deref().unwrap_or(&self.slug)
    }

    /// Whether this product has been published to Shopify (and is therefore
    /// buyable). We treat a Printify sync as the signal it reached Shopify.
    pub fn is_live(&self) -> bool {
        self.printify_product_id.is_some()
    }
}

impl Product {
    /// First line of the (possibly multi-line) slogan — used for card titles.
    pub fn slogan_first_line(&self) -> &str {
        self.slogan.split('\n').next().unwrap_or(&self.slogan)
    }

    /// Slogan collapsed to a single line.
    pub fn slogan_flat(&self) -> String {
        self.slogan.replace('\n', " ")
    }
}
