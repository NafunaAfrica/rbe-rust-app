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

pub async fn connect(cfg: &crate::config::Config) -> anyhow::Result<Surreal<Db>> {
    let db = Surreal::new::<SurrealKv>(&cfg.data_dir).await?;
    db.use_ns(NS).use_db(DB).await?;
    define_schema(&db).await?;
    seed_products(&db).await?;
    seed_shop_cache(&db).await?;
    seed_staff(&db, cfg).await?;
    seed_posts(&db).await?;
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

        DEFINE TABLE IF NOT EXISTS staff SCHEMALESS;
        DEFINE FIELD IF NOT EXISTS email ON staff TYPE string;
        DEFINE INDEX IF NOT EXISTS staff_email ON staff FIELDS email UNIQUE;

        DEFINE TABLE IF NOT EXISTS customer SCHEMALESS;
        DEFINE FIELD IF NOT EXISTS email ON customer TYPE string;
        DEFINE INDEX IF NOT EXISTS customer_email ON customer FIELDS email UNIQUE;

        DEFINE TABLE IF NOT EXISTS post SCHEMALESS;
        DEFINE FIELD IF NOT EXISTS slug ON post TYPE string;
        DEFINE INDEX IF NOT EXISTS post_slug ON post FIELDS slug UNIQUE;

        DEFINE TABLE IF NOT EXISTS order SCHEMALESS;

        DEFINE TABLE IF NOT EXISTS pageview SCHEMALESS;
        "#,
    )
    .await?
    .check()?;
    Ok(())
}

/// Seed the admin staff account from env (`RBE_ADMIN_EMAIL` / `RBE_ADMIN_PASSWORD`)
/// on first boot. Password is stored as an argon2 hash; if the admin already
/// exists we leave it untouched (change it in-app, not via env).
async fn seed_staff(db: &Surreal<Db>, cfg: &crate::config::Config) -> anyhow::Result<()> {
    let email = cfg.admin_email.trim().to_lowercase();
    let existing: Option<crate::models::Staff> = db
        .query("SELECT * FROM staff WHERE email = $e LIMIT 1")
        .bind(("e", email.clone()))
        .await?
        .take(0)?;
    if existing.is_some() {
        return Ok(());
    }
    let hash = crate::auth::hash_password(&cfg.admin_password)?;
    db.query("CREATE staff CONTENT { email: $email, password_hash: $hash, role: 'admin', created_at: time::now() }")
        .bind(("email", email))
        .bind(("hash", hash))
        .await?
        .check()?;
    tracing::info!("seeded admin staff account");
    Ok(())
}

/// Seed a few published journal posts on first boot so `/journal` isn't empty.
async fn seed_posts(db: &Surreal<Db>) -> anyhow::Result<()> {
    let existing: Vec<crate::models::Post> =
        db.query("SELECT * FROM post LIMIT 1").await?.take(0)?;
    if !existing.is_empty() {
        return Ok(());
    }
    let seed: [(&str, &str, &str, &str, &str); 3] = [
        (
            "on-being-called-intimidating",
            "On being called 'intimidating' one more time",
            "Essay",
            "2026-07-20T09:00:00Z",
            "They call it intimidating because *ambitious* would give it away.\n\nRBE is for the ones who stopped shrinking to fit the room — and started buying bigger rooms.\n\n> We do not apologize for our tone.\n\nWear the thesis. Mean it.",
        ),
        (
            "soft-life-spreadsheet",
            "The soft-life spreadsheet that saved my summer",
            "Money",
            "2026-07-13T09:00:00Z",
            "A soft life is a **budgeted** life. Here's the one-tab system:\n\n- Pay yourself first\n- Automate the boring bills\n- A line for joy, on purpose\n\nMoney is just quiet confidence you can spend.",
        ),
        (
            "playlist-for-closing-deals",
            "A playlist for closing tabs and closing deals",
            "Sound",
            "2026-07-06T09:00:00Z",
            "Sound is a strategy. This is the set we play while invoicing:\n\n1. Something that struts\n2. Something that focuses\n3. Something that celebrates\n\nPress play. Send the invoice.",
        ),
    ];
    for (slug, title, tag, published_at, body) in seed {
        db.query(
            "CREATE type::thing('post', $slug) CONTENT { slug: $slug, title: $title, excerpt: $excerpt, tag: $tag, \
             body_md: $body, status: 'published', author: 'RBE', published_at: $pub, updated_at: $pub }",
        )
        .bind(("slug", slug.to_string()))
        .bind(("title", title.to_string()))
        .bind(("excerpt", body.lines().next().unwrap_or("").to_string()))
        .bind(("tag", tag.to_string()))
        .bind(("body", body.to_string()))
        .bind(("pub", published_at.to_string()))
        .await?
        .check()?;
    }
    tracing::info!("seeded journal posts");
    Ok(())
}

/// Create or update a post by slug (used by the journal editor).
pub async fn upsert_post(
    db: &Surreal<Db>,
    slug: &str,
    title: &str,
    excerpt: &str,
    cover_url: Option<&str>,
    tag: &str,
    body_md: &str,
    status: &str,
    author: &str,
    now_rfc3339: &str,
) -> anyhow::Result<()> {
    // Preserve the original publish date; set it the first time it goes live.
    let existing: Option<crate::models::Post> = db
        .query("SELECT * FROM type::thing('post', $slug)")
        .bind(("slug", slug.to_string()))
        .await?
        .take(0)?;
    let prior_published = existing.and_then(|p| p.published_at);
    let published_at = if status == "published" {
        prior_published.or_else(|| Some(now_rfc3339.to_string()))
    } else {
        prior_published
    };

    db.query(
        "UPSERT type::thing('post', $slug) SET \
         slug = $slug, title = $title, excerpt = $excerpt, cover_url = $cover, tag = $tag, \
         body_md = $body, status = $status, author = $author, updated_at = $now, published_at = $pub",
    )
    .bind(("slug", slug.to_string()))
    .bind(("title", title.to_string()))
    .bind(("excerpt", excerpt.to_string()))
    .bind(("cover", cover_url.map(|s| s.to_string())))
    .bind(("tag", tag.to_string()))
    .bind(("body", body_md.to_string()))
    .bind(("status", status.to_string()))
    .bind(("author", author.to_string()))
    .bind(("now", now_rfc3339.to_string()))
    .bind(("pub", published_at))
    .await?
    .check()?;
    Ok(())
}

/// Record a page view (first-party analytics). `day` is derived from the
/// timestamp for cheap per-day grouping.
pub async fn record_pageview(
    db: &Surreal<Db>,
    path: &str,
    session: &str,
    referrer: Option<&str>,
) -> anyhow::Result<()> {
    let ts = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let day = ts.get(0..10).unwrap_or("").to_string();
    db.query("CREATE pageview CONTENT { path: $path, session: $sid, referrer: $ref, ts: $ts, day: $day }")
        .bind(("path", path.to_string()))
        .bind(("sid", session.to_string()))
        .bind(("ref", referrer.map(|s| s.to_string())))
        .bind(("ts", ts))
        .bind(("day", day))
        .await?
        .check()?;
    Ok(())
}

/// Insert or replace an order (from a Shopify webhook). Keyed by the Shopify
/// order id so repeated `orders/updated` events overwrite the same record.
pub async fn upsert_order(db: &Surreal<Db>, order: &crate::models::Order) -> anyhow::Result<()> {
    db.query("UPSERT type::thing('order', $id) CONTENT $data")
        .bind(("id", order.shopify_order_id.clone()))
        .bind(("data", order.clone()))
        .await?
        .check()?;
    Ok(())
}

/// Create a new staff member (used by the admin "Team" screen).
pub async fn create_staff(
    db: &Surreal<Db>,
    email: &str,
    password_hash: &str,
    role: &str,
) -> anyhow::Result<()> {
    db.query("CREATE staff CONTENT { email: $email, password_hash: $hash, role: $role, created_at: time::now() }")
        .bind(("email", email.trim().to_lowercase()))
        .bind(("hash", password_hash.to_string()))
        .bind(("role", role.to_string()))
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
