# RBE — Rich B Energy

A single-binary web app for the RBE slogan-tee store. Rebuilt from a Lovable
(TanStack Start + Supabase) export into a Rust stack, deployable as **one
container** with **no Node runtime**.

## Stack

| Concern | Choice |
|---|---|
| HTTP server | **Axum** (Tokio) |
| HTML | **Maud** server-side templates (SSR) |
| Interactivity | **HTMX** + **Alpine.js** (vendored, no build step) |
| Styling | **Tailwind v4** via the standalone CLI (no Node) |
| Database | **SurrealDB**, embedded in-process (SurrealKV engine) |
| Catalog + checkout | **Shopify** Storefront API (server-side) |
| Fulfillment | **Printify** API (print-on-demand) |
| Admin auth | Email + password → JWT session cookie |
| Site agent | **rig** (Anthropic/Claude) — scaffolded in `src/agent.rs`, built out later |

The app talks to Shopify and Printify over HTTPS; it does **not** replace them.
SurrealDB replaces what Supabase did: internal product/design metadata, the
`shop_cache` version counter, and (via SSE) live cache invalidation.

## Layout

```
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
    shopify.rs    Storefront GraphQL client
    printify.rs   Printify client + product sync
  web/
    layout.rs     base shell + cart drawer (Alpine)
    home.rs shop.rs pages.rs auth_page.rs admin.rs
    mod.rs        router + tee_mockup helper
styles/input.css  Tailwind entry (compiled to static/app.css)
static/           app.css, vendored JS, images, favicon
reference/app/    the original Lovable export (kept for reference)
```

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

## Routes

| Method | Path | Purpose |
|---|---|---|
| GET | `/` | Home |
| GET | `/shop`, `/shop/{slug}` | Storefront (Shopify) |
| GET | `/manifesto`, `/journal` | Content |
| POST | `/api/checkout` | Create Shopify cart → checkout URL |
| GET | `/events` | SSE cache-bust stream |
| GET/POST | `/auth`, `/auth/logout` | Admin login |
| GET | `/admin`, `/admin/printify` | Admin (JWT-gated) |
| GET | `/admin/printify/sync` | SSE live sync log |
| POST | `/api/webhooks/{printify,shopify}` | HMAC-verified webhooks |

## Agent (next)

`src/agent.rs` wires a Claude-backed `rig` agent with a role preamble. It's a
skeleton: the next milestone registers `rig` **tools** (sync a product, edit
product metadata in SurrealDB, draft journal posts) and an admin chat UI so the
agent can manage the site. Set `ANTHROPIC_API_KEY` to enable it.
