//! Printify API client + product sync (server-side port of the reference app's
//! `src/lib/printify.server.ts` and `printify.functions.ts`).

use serde::{Deserialize, Serialize};
use serde_json::json;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::models::Product;

const BASE_URL: &str = "https://api.printify.com/v1";
const DEFAULT_BLUEPRINT_ID: i64 = 12;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Shop {
    pub id: serde_json::Value, // Printify returns numeric or string ids
    pub title: String,
    pub sales_channel: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Variant {
    id: i64,
    options: VariantOptions,
}

#[derive(Debug, Deserialize)]
struct VariantOptions {
    color: String,
    #[allow(dead_code)]
    size: String,
}

#[derive(Debug, Deserialize)]
struct ProviderVariants {
    variants: Vec<Variant>,
}

/// One line of the live sync log streamed to the admin page.
#[derive(Debug, Clone, Serialize)]
pub struct SyncStep {
    pub name: String,
    pub status: String, // "ok" | "error" | "info"
    pub detail: String,
}

impl SyncStep {
    fn new(name: &str, status: &str, detail: impl Into<String>) -> Self {
        SyncStep {
            name: name.into(),
            status: status.into(),
            detail: detail.into(),
        }
    }
}

fn color_aliases(hex: &str) -> Vec<&'static str> {
    match hex.to_lowercase().trim() {
        "#ffffff" => vec!["White"],
        "#0a0a0a" => vec!["Black"],
        "#ffc7dd" => vec!["Soft Pink", "Pink"],
        "#ff2b8f" => vec!["Soft Pink", "Pink", "Red"],
        "#f5e9d4" => vec!["Natural", "Maize Yellow"],
        _ => vec!["White"],
    }
}

pub struct Printify<'a> {
    cfg: &'a Config,
    http: &'a reqwest::Client,
}

impl<'a> Printify<'a> {
    pub fn new(cfg: &'a Config, http: &'a reqwest::Client) -> Self {
        Printify { cfg, http }
    }

    fn token(&self) -> AppResult<&str> {
        self.cfg
            .printify_api_token
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("PRINTIFY_API_TOKEN is not configured".into()))
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> AppResult<serde_json::Value> {
        let token = self.token()?;
        let mut req = self
            .http
            .request(method, format!("{BASE_URL}{path}"))
            .bearer_auth(token)
            .header("Content-Type", "application/json");
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "Printify API error {status}: {text}"
            )));
        }
        Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
    }

    pub async fn shops(&self) -> AppResult<Vec<Shop>> {
        let v = self.request(reqwest::Method::GET, "/shops.json", None).await?;
        serde_json::from_value(v).map_err(|e| AppError::Other(e.into()))
    }

    async fn print_providers(&self, blueprint_id: i64) -> AppResult<Vec<serde_json::Value>> {
        let v = self
            .request(
                reqwest::Method::GET,
                &format!("/catalog/blueprints/{blueprint_id}/print_providers.json"),
                None,
            )
            .await?;
        serde_json::from_value(v).map_err(|e| AppError::Other(e.into()))
    }

    async fn provider_variants(
        &self,
        blueprint_id: i64,
        provider_id: i64,
    ) -> AppResult<ProviderVariants> {
        let v = self
            .request(
                reqwest::Method::GET,
                &format!(
                    "/catalog/blueprints/{blueprint_id}/print_providers/{provider_id}/variants.json"
                ),
                None,
            )
            .await?;
        serde_json::from_value(v).map_err(|e| AppError::Other(e.into()))
    }

    async fn upload_image(&self, shop_id: &str, url: &str, filename: &str) -> AppResult<String> {
        let v = self
            .request(
                reqwest::Method::POST,
                &format!("/shops/{shop_id}/uploads.json"),
                Some(json!({ "file_name": filename, "url": url })),
            )
            .await?;
        Ok(v["id"].as_str().unwrap_or_default().to_string())
    }

    /// Sync a single product to Printify. Returns (success, action, steps).
    /// Ported closely from the reference `syncOneProductToPrintify`.
    pub async fn sync_one(
        &self,
        db: &Surreal<Db>,
        shop_id: &str,
        slug: &str,
    ) -> (bool, String, Vec<SyncStep>) {
        let mut steps = Vec::new();
        match self.sync_inner(db, shop_id, slug, &mut steps).await {
            Ok(action) => (true, action, steps),
            Err(e) => {
                steps.push(SyncStep::new("error", "error", e.to_string()));
                (false, "failed".into(), steps)
            }
        }
    }

    async fn sync_inner(
        &self,
        db: &Surreal<Db>,
        shop_id: &str,
        slug: &str,
        steps: &mut Vec<SyncStep>,
    ) -> AppResult<String> {
        steps.push(SyncStep::new("load-product", "info", format!("Loading {slug}")));
        let product: Option<Product> = db
            .query("SELECT * FROM type::thing('product', $slug)")
            .bind(("slug", slug.to_string()))
            .await?
            .take(0)?;
        let product = product.ok_or_else(|| AppError::NotFound)?;
        steps.push(SyncStep::new(
            "load-product",
            "ok",
            format!("tee_color={}", product.tee_color),
        ));

        if let Some(pid) = &product.printify_product_id {
            if product.printify_shop_id.as_deref() == Some(shop_id) {
                steps.push(SyncStep::new("skip", "info", format!("Already synced ({pid})")));
                return Ok("skipped".into());
            }
        }
        let action = if product.printify_product_id.is_some() {
            "updated"
        } else {
            "created"
        };

        steps.push(SyncStep::new("print-provider", "info", format!("Blueprint {DEFAULT_BLUEPRINT_ID}")));
        let providers = self.print_providers(DEFAULT_BLUEPRINT_ID).await?;
        let provider_id = providers
            .first()
            .and_then(|p| p["id"].as_i64())
            .ok_or_else(|| AppError::BadRequest("No print provider for blueprint".into()))?;
        steps.push(SyncStep::new("print-provider", "ok", format!("provider={provider_id}")));

        steps.push(SyncStep::new("variants", "info", "Fetching variants"));
        let pv = self.provider_variants(DEFAULT_BLUEPRINT_ID, provider_id).await?;
        let aliases = color_aliases(&product.tee_color);
        let matched: Vec<i64> = pv
            .variants
            .iter()
            .filter(|v| aliases.contains(&v.options.color.as_str()))
            .map(|v| v.id)
            .collect();
        if matched.is_empty() {
            return Err(AppError::BadRequest(format!(
                "No matching variants for tee_color={}",
                product.tee_color
            )));
        }
        steps.push(SyncStep::new("variants", "ok", format!("{} variant(s) matched", matched.len())));

        // NOTE: design assets in the reference app were hosted on Lovable's CDN.
        // For now we hand Printify the local mockup image; swap for a real design
        // asset URL when designs are re-hosted.
        let image_url = format!("{}{}", self.cfg.public_base_url, product.image);
        steps.push(SyncStep::new("upload", "info", image_url.clone()));
        let image_id = self.upload_image(shop_id, &image_url, &format!("{slug}.png")).await?;
        steps.push(SyncStep::new("upload", "ok", format!("image_id={image_id}")));

        steps.push(SyncStep::new("create", "info", "Creating Printify product"));
        let variants_json: Vec<_> = matched
            .iter()
            .map(|id| json!({ "id": id, "price": product.price * 100 }))
            .collect();
        let create_body = json!({
            "title": format!("RBE — {}", product.slogan_flat()),
            "description": product.description,
            "blueprint_id": DEFAULT_BLUEPRINT_ID,
            "print_provider_id": provider_id,
            "variants": variants_json,
            "print_areas": [{
                "variant_ids": matched,
                "placeholders": [{
                    "position": "front",
                    "images": [{ "id": image_id, "x": 0.5, "y": 0.5, "scale": product.scale, "angle": 0 }]
                }]
            }]
        });
        let created = self
            .request(
                reqwest::Method::POST,
                &format!("/shops/{shop_id}/products.json"),
                Some(create_body),
            )
            .await?;
        let printify_id = created["id"].to_string().trim_matches('"').to_string();
        steps.push(SyncStep::new("create", "ok", format!("printify_id={printify_id}")));

        steps.push(SyncStep::new("publish", "info", "Publishing to sales channel"));
        let publish = self
            .request(
                reqwest::Method::POST,
                &format!("/shops/{shop_id}/products/{printify_id}/publish.json"),
                Some(json!({
                    "title": true, "description": true, "images": true, "variants": true,
                    "tags": true, "keyFeatures": true, "shipping_template": true
                })),
            )
            .await;
        match publish {
            Ok(_) => steps.push(SyncStep::new("publish", "ok", "Publish requested")),
            Err(e) => steps.push(SyncStep::new("publish", "error", e.to_string())),
        }

        steps.push(SyncStep::new("save", "info", "Updating DB"));
        db.query(
            "UPDATE type::thing('product', $slug) SET \
             printify_product_id = $pid, printify_status = $status, printify_shop_id = $shop",
        )
        .bind(("slug", slug.to_string()))
        .bind(("pid", printify_id.clone()))
        .bind(("status", created["status"].as_str().unwrap_or("").to_string()))
        .bind(("shop", shop_id.to_string()))
        .await?
        .check()?;
        steps.push(SyncStep::new("save", "ok", "Saved"));

        Ok(action.into())
    }
}
