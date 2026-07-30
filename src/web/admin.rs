//! Admin control panel: dashboard, Printify sync (with a live SSE log), and a
//! placeholder for the forthcoming rig-powered agent.

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Html;
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth::AdminUser;
use crate::error::AppResult;
use crate::models::Product;
use crate::services::printify::Printify;
use crate::state::AppState;

use super::layout::{Nav, shell};

pub async fn dashboard(_admin: AdminUser) -> Html<String> {
    let body = html! {
        div class="mx-auto max-w-3xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Control panel" }
                a href="/auth/logout" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "Sign out" }
            }
            div class="mt-10 grid gap-4 sm:grid-cols-2" {
                (admin_card("Printify sync", "Push RBE tee designs to Printify as live print-on-demand listings.", "/admin/printify"))
                (admin_card("Agent (soon)", "The rig-powered site agent will manage products, sync, and content from here.", "#"))
            }
        }
    };
    Html(shell("Admin — RBE", "RBE admin control panel.", Nav::None, body).into_string())
}

fn admin_card(title: &str, desc: &str, href: &str) -> Markup {
    html! {
        a href=(href) class="block rounded-lg border border-ink/10 bg-white p-6 transition hover:border-[color:var(--hot)] hover:shadow-lg hover:shadow-[color:var(--hot)]/10" {
            div class="font-display text-2xl" { (title) }
            p class="mt-2 text-sm opacity-70" { (desc) }
        }
    }
}

pub async fn printify_page(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let products: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug")
        .await?
        .take(0)?;

    // Shops require a Printify token; degrade gracefully if not configured.
    let printify = Printify::new(state.cfg(), state.http());
    let shops = printify.shops().await.unwrap_or_default();

    let synced = products.iter().filter(|p| p.printify_product_id.is_some()).count();

    let body = html! {
        div class="mx-auto max-w-3xl px-4 py-16" x-data="printifySync()" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Printify Sync" }
                a href="/admin" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "← Admin" }
            }
            p class="mt-2 text-muted-foreground" { "Sync the RBE tee designs to your Printify store as live POD listings." }

            div class="mt-8" {
                label class="text-sm font-medium" { "Printify shop" }
                @if shops.is_empty() {
                    p class="mt-2 text-sm opacity-60" { "No shops (set PRINTIFY_API_TOKEN to connect)." }
                } @else {
                    select "x-model"="shopId" class="mt-2 block w-full max-w-md rounded-md border border-ink/20 bg-white px-3 py-2 text-sm" {
                        @for s in &shops {
                            @let id = s.id.to_string().trim_matches('"').to_string();
                            option value=(id) {
                                (s.title) " · " (s.sales_channel.clone().unwrap_or_default()) " · id " (id)
                            }
                        }
                    }
                }
            }

            div class="mt-8 grid grid-cols-3 gap-4" {
                (stat("Total", &products.len().to_string()))
                (stat("Synced", &synced.to_string()))
                (stat("Pending", &(products.len() - synced).to_string()))
            }

            div class="mt-8 flex flex-wrap gap-3" {
                button "@click"="run('pending')" ":disabled"="running || !shopId"
                    class="rounded-full bg-[color:var(--hot)] px-5 py-2 text-sm font-semibold uppercase tracking-widest text-white disabled:opacity-40" {
                    "Sync pending"
                }
                button "@click"="run('all')" ":disabled"="running || !shopId"
                    class="rounded-full border border-ink/20 px-5 py-2 text-sm font-semibold uppercase tracking-widest disabled:opacity-40" {
                    "Sync all"
                }
                span x-show="running" class="self-center text-sm opacity-60" { "Running…" }
            }

            div class="mt-8" {
                h2 class="font-semibold" { "Products (" (products.len()) ")" }
                ul class="mt-2 divide-y" {
                    @for p in &products {
                        li class="flex items-center justify-between py-2" {
                            div {
                                div class="font-medium" { (p.slogan_flat()) }
                                div class="text-xs opacity-60" { "$" (p.price) }
                            }
                            div class="text-sm" {
                                @if p.printify_product_id.is_some() {
                                    span class="text-green-600" { "Synced" }
                                } @else {
                                    span class="text-amber-600" { "Pending" }
                                }
                            }
                        }
                    }
                }
            }

            div class="mt-8" {
                h2 class="font-semibold" { "Sync log" }
                div class="mt-2 h-80 overflow-y-auto rounded-md border bg-white/60 p-3 font-mono text-xs" {
                    template x-if="log.length === 0" {
                        p class="opacity-60" { "No activity yet. Run a sync to see step-by-step progress here." }
                    }
                    template x-for="(l, i) in log" ":key"="i" {
                        div ":class"="l.status === 'error' ? 'text-red-600' : (l.status === 'ok' ? 'text-green-700' : 'opacity-60')" {
                            span class="font-semibold" { "[" span x-text="l.slug" {} "] " }
                            span class="uppercase" x-text="l.name" {}
                            span { " — " } span x-text="l.detail" {}
                        }
                    }
                }
            }
        }
        (sync_script())
    };

    Ok(Html(shell("Printify Sync — RBE Admin", "Sync RBE products to Printify.", Nav::None, body).into_string()))
}

fn stat(label: &str, value: &str) -> Markup {
    html! {
        div class="rounded-lg border border-ink/10 bg-white p-4" {
            div class="text-2xl font-semibold" { (value) }
            div class="text-xs uppercase tracking-widest opacity-60" { (label) }
        }
    }
}

/// SSE endpoint: streams one event per sync step, then a `done` event.
#[derive(Deserialize)]
pub struct SyncQuery {
    shop: String,
    /// "pending" | "all"
    scope: String,
}

pub async fn printify_sync_stream(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(q): Query<SyncQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let products: Vec<Product> = match state
            .db()
            .query("SELECT * FROM product ORDER BY slug")
            .await
            .and_then(|mut r| r.take(0))
        {
            Ok(p) => p,
            Err(e) => {
                let _ = tx
                    .send(Ok(Event::default().event("error").data(e.to_string())))
                    .await;
                return;
            }
        };

        let printify = Printify::new(state.cfg(), state.http());
        let targets: Vec<Product> = products
            .into_iter()
            .filter(|p| q.scope == "all" || p.printify_product_id.is_none())
            .collect();

        let mut ok = 0;
        let mut failed = 0;
        for product in &targets {
            let (success, _action, steps) =
                printify.sync_one(state.db(), &q.shop, &product.slug).await;
            for s in steps {
                let payload = json!({ "slug": product.slug, "name": s.name, "status": s.status, "detail": s.detail });
                let _ = tx.send(Ok(Event::default().event("step").data(payload.to_string()))).await;
            }
            if success { ok += 1 } else { failed += 1 }
        }

        let summary = json!({ "ok": ok, "failed": failed, "total": targets.len() });
        let _ = tx.send(Ok(Event::default().event("done").data(summary.to_string()))).await;
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

fn sync_script() -> Markup {
    maud::PreEscaped(
        r#"<script>
function printifySync(){
  return {
    shopId: '',
    running: false,
    log: [],
    run(scope){
      if (!this.shopId || this.running) return;
      this.running = true; this.log = [];
      const es = new EventSource(`/admin/printify/sync?shop=${encodeURIComponent(this.shopId)}&scope=${scope}`);
      es.addEventListener('step', (e) => this.log.push(JSON.parse(e.data)));
      es.addEventListener('error', (e) => { this.log.push({ slug:'-', name:'error', status:'error', detail:'stream error' }); });
      es.addEventListener('done', (e) => { this.running = false; es.close(); });
    }
  };
}
</script>"#.to_string(),
    )
}
