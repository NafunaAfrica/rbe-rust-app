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

impl Product {
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
