# RBE - Rich B Energy

A single-binary web app for the RBE slogan-tee store. Rebuilt from a Lovable
(TanStack Start + Supabase) export into a Rust stack, deployable as **one
container** with **no Node runtime**.

As of **August 1, 2026**, the app is in a hybrid commerce state:

- The storefront catalog is still rendered from the app's own SurrealDB `product` table.
- Shopify is the live source for checkout visibility and live variant resolution.
- Shopify webhooks can now sync selected product fields back into the local catalog.
- New Shopify products do **not** automatically appear on the storefront yet unless they are added to the local catalog or the storefront is migrated to read directly from Shopify.

## Stack

| Concern | Choice |
|---|---|
| HTTP server | **Axum** (Tokio) |
| HTML | **Maud** server-side templates (SSR) |
| Interactivity | **HTMX** + **Alpine.js** (vendored, no build step) |
| Styling | **Tailwind v4** via the standalone CLI (no Node) |
| Database | **SurrealDB**, embedded in-process (SurrealKV engine) |
| Catalog + checkout | **Shopify** Storefront API + Admin API (server-side) |
| Fulfillment | Transitioning away from **Printify** |
| Admin auth | Email + password -> JWT session cookie |
| Site agent | **rig** (Anthropic/Claude) - scaffolded in `src/agent.rs`, built out later |

The app talks to Shopify and Printify over HTTPS; it does **not** replace them.
SurrealDB replaces what Supabase did: internal product/design metadata, the
`shop_cache` version counter, order ingestion, and live cache invalidation.

## Layout

```text
src/
  main.rs         server bootstrap
  config.rs       env-driven config
  db.rs           embedded SurrealDB: connect, schema, seed
  models.rs       Product
  auth.rs         JWT session + admin extractor
  state.rs        shared AppState (db, http, event bus)
  error.rs        AppError -> HTTP response
  events.rs       SSE cache-bust stream
  webhooks.rs     Printify + Shopify HMAC webhooks
  agent.rs        rig site-agent scaffold
  services/
    shopify.rs    Storefront + Admin GraphQL client
    printify.rs   Printify client + product sync
  web/
    layout.rs     base shell + cart drawer (Alpine)
    home.rs shop.rs pages.rs auth_page.rs admin.rs
    mod.rs        router + tee_mockup helper
styles/input.css  Tailwind entry (compiled to static/app.css)
static/           app.css, vendored JS, images, favicon
reference/app/    the original Lovable export (kept for reference)
```

## How Shopify Mapping Works

Each local product has:

- a local `slug` used by the site URL, bag, and internal catalog
- an optional `shopify_handle` used to look up the real Shopify product

The mapping lives in the `product` table and is represented in `src/models.rs`
and seeded/backfilled in `src/db.rs`.

When a customer checks out:

1. The bag sends local slugs such as `bad-b-club`.
2. The server loads that local product from SurrealDB.
3. The app resolves the Shopify handle using `product.storefront_handle()`.
4. Shopify is queried for that handle.
5. The matching Shopify variant ID is used to build the checkout cart permalink.

So "product A knows product A" because the local product record carries the
Shopify handle mapping. If no `shopify_handle` is set, the app falls back to
using the slug as the Shopify handle.

## What Syncs Automatically

If Shopify webhooks are configured to the live app URL, these things are wired:

- `orders/create` and `orders/updated` can flow back into the local orders dashboard
- `products/create`, `products/update`, and `products/delete` can trigger storefront cache refreshes
- product price can sync from Shopify into the local catalog
- product description can sync from Shopify into the local catalog
- product image can sync from Shopify into the local catalog

Important limitation:

- This does **not** auto-create brand-new storefront cards for new Shopify products yet.
- The storefront is still local-catalog-first, not Shopify-catalog-first.

## Develop (Windows, no Node)

```bash
# 1. Configure secrets
cp .env.example .env   # then fill in values

# 2. Build the CSS (downloads the Tailwind CLI on first run)
./scripts/build-css.ps1

# 3. Run
cargo run
```

App serves on http://localhost:8080. Admin at `/admin` (log in at `/auth` with
`RBE_ADMIN_EMAIL` / `RBE_ADMIN_PASSWORD`).

> Note: `surrealdb-core` is a very large crate. The `[profile.dev]` in
> `Cargo.toml` disables debuginfo so the compiler doesn't run out of memory.
> First build is slow; later builds are incremental.

## Deploy (one container)

```bash
docker compose up --build
```

The multi-stage `Dockerfile` compiles the CSS (standalone Tailwind) and the Rust
binary, then ships a slim image with the binary + static assets. SurrealDB data
persists to the `rbe-data` volume at `/data`.

Before testing Shopify online, make sure your production environment has:

- `RBE_PUBLIC_BASE_URL` set to the real public site URL
- `SHOPIFY_STORE_DOMAIN` set to the correct shop domain
- `SHOPIFY_STOREFRONT_TOKEN` set to the active Storefront token
- `SHOPIFY_API_KEY` or `SHOPIFY_CLIENT_ID` set to the app client ID
- `SHOPIFY_WEBHOOK_SECRET` or `SHOPIFY_CLIENT_SECRET` set to the app secret
- `SHOPIFY_API_VERSION=2026-07`

Without a real `RBE_PUBLIC_BASE_URL`, Shopify webhooks cannot be pointed at the
live app correctly.

## Shopify Webhooks

The live webhook endpoint is:

```text
POST /api/webhooks/shopify
```

Recommended topics for this app:

- `products/create`
- `products/update`
- `products/delete`
- `orders/create`
- `orders/updated`

Official Shopify docs used for the current setup:

- [Manage webhook subscriptions](https://shopify.dev/docs/apps/build/webhooks/subscribe)
- [Webhooks reference](https://shopify.dev/docs/api/webhooks/latest)

## Routes

| Method | Path | Purpose |
|---|---|---|
| GET | `/` | Home |
| GET | `/shop`, `/shop/{slug}` | Storefront |
| GET | `/manifesto`, `/journal` | Content |
| POST | `/api/checkout` | Create Shopify checkout/cart permalink |
| GET | `/events` | SSE cache-bust stream |
| GET/POST | `/auth`, `/auth/logout` | Admin login |
| GET | `/admin`, `/admin/shopify`, `/admin/printify` | Admin (JWT-gated) |
| GET | `/admin/printify/sync` | SSE live sync log |
| POST | `/api/webhooks/{printify,shopify}` | HMAC-verified webhooks |

## Current Admin Surface

The admin area already includes:

- a website management dashboard
- Shopify visibility diagnostics
- order history and fulfilment view
- journal/content management
- team account management

## Agent (next)

`src/agent.rs` wires a Claude-backed `rig` agent with a role preamble. It's a
skeleton: the next milestone registers `rig` tools and an admin chat UI so the
agent can manage the site.
