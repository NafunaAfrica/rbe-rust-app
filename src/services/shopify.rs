//! Shopify Storefront API client (server-side port of the reference app's
//! `src/lib/shopify.ts`). Moving these calls to the backend means the
//! storefront token is no longer shipped to the browser.

use serde::Deserialize;
use serde_json::json;

use crate::config::Config;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
pub struct Money {
    pub amount: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub url: String,
    #[serde(rename = "altText")]
    pub alt_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectedOption {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variant {
    pub id: String,
    #[allow(dead_code)]
    pub title: String,
    pub price: Money,
    #[serde(rename = "availableForSale")]
    pub available_for_sale: bool,
    #[serde(rename = "selectedOptions")]
    pub selected_options: Vec<SelectedOption>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProductOption {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceRange {
    pub min_variant_price: Money,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Product {
    #[allow(dead_code)]
    pub id: String,
    pub title: String,
    pub description: String,
    pub handle: String,
    #[serde(rename = "priceRange")]
    pub price_range: PriceRange,
    pub images: Edges<Image>,
    pub variants: Edges<Variant>,
    pub options: Vec<ProductOption>,
}

/// Generic Shopify `{ edges: [{ node }] }` connection, flattened to a Vec.
#[derive(Debug, Clone)]
pub struct Edges<T>(pub Vec<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Edges<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Node<T> {
            node: T,
        }
        #[derive(Deserialize)]
        struct Conn<T> {
            edges: Vec<Node<T>>,
        }
        let conn = Conn::<T>::deserialize(d)?;
        Ok(Edges(conn.edges.into_iter().map(|n| n.node).collect()))
    }
}

impl Product {
    pub fn first_image(&self) -> Option<&Image> {
        self.images.0.first()
    }
}

pub struct Shopify<'a> {
    cfg: &'a Config,
    http: &'a reqwest::Client,
}

impl<'a> Shopify<'a> {
    pub fn new(cfg: &'a Config, http: &'a reqwest::Client) -> Self {
        Shopify { cfg, http }
    }

    fn endpoint(&self) -> String {
        format!(
            "https://{}/api/{}/graphql.json",
            self.cfg.shopify_store_domain, self.cfg.shopify_api_version
        )
    }

    async fn request(&self, query: &str, variables: serde_json::Value) -> AppResult<serde_json::Value> {
        let resp = self
            .http
            .post(self.endpoint())
            .header(
                "X-Shopify-Storefront-Access-Token",
                &self.cfg.shopify_storefront_token,
            )
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await?;

        if resp.status().as_u16() == 402 {
            return Err(AppError::BadRequest(
                "Shopify API access requires an active billing plan".into(),
            ));
        }
        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;
        if let Some(errors) = body.get("errors").filter(|e| !e.is_null()) {
            return Err(AppError::BadRequest(format!("Shopify: {errors}")));
        }
        if !status.is_success() {
            return Err(AppError::BadRequest(format!("Shopify HTTP {status}")));
        }
        Ok(body["data"].clone())
    }

    pub async fn products(&self, first: i32) -> AppResult<Vec<Product>> {
        let data = self.request(PRODUCTS_QUERY, json!({ "first": first })).await?;
        let edges: Edges<Product> = serde_json::from_value(data["products"].clone())
            .map_err(|e| AppError::Other(e.into()))?;
        Ok(edges.0)
    }

    pub async fn product_by_handle(&self, handle: &str) -> AppResult<Option<Product>> {
        let data = self
            .request(PRODUCT_BY_HANDLE_QUERY, json!({ "handle": handle }))
            .await?;
        if data["product"].is_null() {
            return Ok(None);
        }
        let p: Product = serde_json::from_value(data["product"].clone())
            .map_err(|e| AppError::Other(e.into()))?;
        Ok(Some(p))
    }

    /// Create a Shopify cart and return its hosted checkout URL.
    pub async fn create_cart(&self, lines: &[(String, i32)]) -> AppResult<String> {
        let line_input: Vec<_> = lines
            .iter()
            .map(|(id, qty)| json!({ "merchandiseId": id, "quantity": qty }))
            .collect();
        let data = self
            .request(CART_CREATE_MUTATION, json!({ "lines": line_input }))
            .await?;
        let cart = &data["cartCreate"]["cart"];
        if cart.is_null() {
            let errs = &data["cartCreate"]["userErrors"];
            return Err(AppError::BadRequest(format!("Cart create failed: {errs}")));
        }
        Ok(cart["checkoutUrl"].as_str().unwrap_or_default().to_string())
    }
}

pub fn format_money(amount: &str, currency: &str) -> String {
    let value: f64 = amount.parse().unwrap_or(0.0);
    format!("{currency} {value:.2}")
}

const PRODUCTS_QUERY: &str = r#"
query GetProducts($first: Int!) {
  products(first: $first) {
    edges { node {
      id title description handle
      priceRange { minVariantPrice { amount currencyCode } }
      images(first: 5) { edges { node { url altText } } }
      variants(first: 50) { edges { node { id title price { amount currencyCode } availableForSale selectedOptions { name value } } } }
      options { name values }
    } }
  }
}"#;

const PRODUCT_BY_HANDLE_QUERY: &str = r#"
query GetProductByHandle($handle: String!) {
  product(handle: $handle) {
    id title description handle
    priceRange { minVariantPrice { amount currencyCode } }
    images(first: 10) { edges { node { url altText } } }
    variants(first: 100) { edges { node { id title price { amount currencyCode } availableForSale selectedOptions { name value } } } }
    options { name values }
  }
}"#;

const CART_CREATE_MUTATION: &str = r#"
mutation CartCreate($lines: [CartLineInput!]!) {
  cartCreate(input: { lines: $lines }) {
    cart { id checkoutUrl }
    userErrors { field message }
  }
}"#;
