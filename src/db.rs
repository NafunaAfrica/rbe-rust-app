//! Embedded SurrealDB: connection, schema definition, and initial seed.
//!
//! We run SurrealDB *in-process* using the SurrealKV storage engine, persisting
//! to a directory on disk. No external database server — the whole app is one
//! binary, which is what makes the single-container deploy honest.

use surrealdb::Surreal;
use surrealdb::engine::local::{Db, SurrealKv};

use crate::models::Product;

/// Namespace / database names inside SurrealDB.
const NS: &str = "rbe";
const DB: &str = "rbe";

pub async fn connect(data_dir: &str) -> anyhow::Result<Surreal<Db>> {
    let db = Surreal::new::<SurrealKv>(data_dir).await?;
    db.use_ns(NS).use_db(DB).await?;
    define_schema(&db).await?;
    seed_products(&db).await?;
    seed_shop_cache(&db).await?;
    Ok(db)
}

/// Idempotent schema. SurrealDB `DEFINE ... IF NOT EXISTS` lets us run this on
/// every boot without migrations.
async fn define_schema(db: &Surreal<Db>) -> anyhow::Result<()> {
    db.query(
        r#"
        DEFINE TABLE IF NOT EXISTS product SCHEMALESS;
        DEFINE FIELD IF NOT EXISTS slug ON product TYPE string;
        DEFINE INDEX IF NOT EXISTS product_slug ON product FIELDS slug UNIQUE;

        DEFINE TABLE IF NOT EXISTS shop_cache SCHEMALESS;
        DEFINE FIELD IF NOT EXISTS version ON shop_cache TYPE int;
        DEFINE FIELD IF NOT EXISTS source ON shop_cache TYPE option<string>;
        DEFINE FIELD IF NOT EXISTS updated_at ON shop_cache TYPE datetime;
        "#,
    )
    .await?
    .check()?;
    Ok(())
}

/// Ensure the `shop_cache:products` version row exists. The CREATE errors
/// harmlessly if the record is already there, so we swallow the result.
async fn seed_shop_cache(db: &Surreal<Db>) -> anyhow::Result<()> {
    let _ = db
        .query("CREATE shop_cache:products SET version = 1, source = 'seed', updated_at = time::now()")
        .await;
    Ok(())
}

/// Seed the internal product/design table from the original hard-coded catalog
/// (ported from the reference app's `src/lib/products.ts`). Only inserts rows
/// that don't already exist, so it's safe on every boot.
async fn seed_products(db: &Surreal<Db>) -> anyhow::Result<()> {
    let existing: Vec<Product> = db.query("SELECT * FROM product LIMIT 1").await?.take(0)?;
    if !existing.is_empty() {
        return Ok(());
    }

    for p in seed_data() {
        let _: Option<Product> = db
            .query("CREATE type::thing('product', $slug) CONTENT $data")
            .bind(("slug", p.slug.clone()))
            .bind(("data", p))
            .await?
            .take(0)?;
    }
    tracing::info!("seeded product table");
    Ok(())
}

fn product(
    slug: &str,
    slogan: &str,
    price: i64,
    tee: &str,
    ink: &str,
    font: &str,
    scale: f64,
    description: &str,
    vibe: &str,
    image: &str,
) -> Product {
    Product {
        slug: slug.into(),
        slogan: slogan.into(),
        price,
        tee_color: tee.into(),
        ink_color: ink.into(),
        font_class: font.into(),
        scale,
        description: description.into(),
        vibe: vibe.into(),
        image: format!("/static/img/{image}"),
        printify_product_id: None,
        printify_status: None,
        printify_shop_id: None,
    }
}

fn seed_data() -> Vec<Product> {
    const D: &str = "font-display";
    const S: &str = "font-serif-display";
    vec![
        product("rich-b-energy", "RICH B\nENERGY", 42, "#ffffff", "#e60023", D, 1.1,
            "The flagship. Loud, unapologetic, iconic. Heavyweight 100% cotton.", "Flagship", "tee-rich-b-energy.jpg"),
        product("hot-girls-go-to-therapy", "Hot Girls\nGo To Therapy", 38, "#ffc7dd", "#c1153f", S, 1.0,
            "Soft pink tee, red script. For the ones who did the work.", "Soft power", "tee-hot-girls-therapy.jpg"),
        product("main-character", "MAIN\nCHARACTER", 38, "#ffffff", "#e60023", S, 1.0,
            "White tee, red serif print. You're the plot.", "Lead role", "tee-main-character.jpg"),
        product("im-literally-just-a-girl", "I'm literally\njust a girl", 36, "#ff2b8f", "#ffffff", S, 1.0,
            "Hot pink, cream serif. The universal defense.", "Serve", "tee-just-a-girl.jpg"),
        product("all-sugar-no-daddy", "ALL SUGAR\nNO DADDY", 40, "#ffffff", "#e60023", D, 1.0,
            "Boxy fit, tiny label detail. Self-made sweetness.", "Self-made", "tee-all-sugar.jpg"),
        product("bad-b-club", "BAD B\nCLUB", 44, "#0a0a0a", "#ff2b8f", D, 1.2,
            "Members only. Black tee, screaming pink ink.", "Members only", "tee-bad-b-club.jpg"),
        product("boring-baby", "I'm boring baby,\nall I do is\nmake money\n& come home.", 40, "#ffffff", "#e60023", S, 0.65,
            "For the private ones. White tee, red serif print.", "Grown", "tee-boring-baby.jpg"),
        product("fuck-normal-magic", "F*CK NORMAL\nI WANT\nMAGIC", 42, "#ffffff", "#ff2b8f", D, 0.9,
            "Neon pink graffiti print. For the ones asking for more.", "Manifest", "tee-fuck-normal-magic.jpg"),
        product("call-me-when-youre-rich", "CALL ME WHEN\nYOU'RE RICH", 42, "#f5e9d4", "#4b2e83", D, 1.0,
            "Cream tee, deep purple serif. A boundary, not a suggestion.", "Boundary", "tee-rich-b-energy.jpg"),
        product("father-figure", "FATHER\nFIGURE", 40, "#ffffff", "#c8102e", S, 1.0,
            "White tee, red serif. Provider energy — for her.", "Provider", "tee-main-character.jpg"),
        product("sorry-im-late-kids", "SORRY I'M LATE\nI HAVE KIDS", 40, "#ffffff", "#c8102e", S, 1.0,
            "For the mums who still showed up. White tee, red serif.", "Mum life", "tee-main-character.jpg"),
    ]
}

/// Bump the shop_cache version (called by webhooks). Returns the new version.
pub async fn bump_version(db: &Surreal<Db>, source: &str) -> anyhow::Result<i64> {
    #[derive(serde::Deserialize)]
    struct Row {
        version: i64,
    }
    let rows: Vec<Row> = db
        .query(
            "UPDATE shop_cache:products SET version += 1, source = $source, \
             updated_at = time::now() RETURN AFTER",
        )
        .bind(("source", source.to_string()))
        .await?
        .take(0)?;
    Ok(rows.first().map(|r| r.version).unwrap_or(1))
}
