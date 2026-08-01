//! Orders & fulfilment view for staff (`/dashboard/orders`). Data is ingested
//! from Shopify order webhooks; tracking updates flow back from Shopify.

use axum::extract::State;
use axum::response::Html;
use maud::html;

use crate::auth::StaffUser;
use crate::error::AppResult;
use crate::models::Order;
use crate::state::AppState;

use super::layout::{Nav, shell};

pub async fn orders_list(_user: StaffUser, State(state): State<AppState>) -> AppResult<Html<String>> {
    let orders: Vec<Order> = state
        .db()
        .query("SELECT * FROM order ORDER BY created_at DESC LIMIT 200")
        .await?
        .take(0)?;

    let count = orders.len();
    let revenue: f64 = orders.iter().filter_map(|o| o.total.parse::<f64>().ok()).sum();
    let currency = orders.first().map(|o| o.currency.clone()).unwrap_or_else(|| "USD".into());

    let body = html! {
        div class="mx-auto max-w-4xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Orders" }
                a href="/dashboard" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "← Dashboard" }
            }

            div class="mt-8 grid grid-cols-2 gap-4 sm:grid-cols-3" {
                (stat("Orders", &count.to_string()))
                (stat("Revenue", &format!("{currency} {revenue:.2}")))
                (stat("Awaiting shipment", &orders.iter().filter(|o| o.fulfillment_status.as_deref() != Some("fulfilled")).count().to_string()))
            }

            @if orders.is_empty() {
                div class="mt-8 rounded-lg border border-dashed border-ink/20 bg-white/50 p-10 text-center" {
                    div class="font-display text-2xl" { "No orders yet" }
                    p class="mt-2 mx-auto max-w-md text-sm opacity-70" {
                        "Orders appear here automatically once Shopify order webhooks are pointed at "
                        span class="font-mono" { "/api/webhooks/shopify" } ". Fulfilment updates and tracking then flow back into this table."
                    }
                }
            } @else {
                div class="mt-8 overflow-x-auto rounded-lg border border-ink/10 bg-white" {
                    table class="w-full text-sm" {
                        thead {
                            tr class="border-b border-ink/10 text-left text-xs uppercase tracking-widest opacity-60" {
                                th class="px-4 py-3" { "Order" }
                                th class="px-4 py-3" { "Date" }
                                th class="px-4 py-3" { "Customer" }
                                th class="px-4 py-3 text-right" { "Total" }
                                th class="px-4 py-3" { "Payment" }
                                th class="px-4 py-3" { "Fulfilment" }
                            }
                        }
                        tbody {
                            @for o in &orders {
                                tr class="border-b border-ink/5" {
                                    td class="px-4 py-3 font-medium" { (o.number.clone().unwrap_or_else(|| o.shopify_order_id.clone())) }
                                    td class="px-4 py-3 tabular-nums opacity-70" { (o.created_date()) }
                                    td class="px-4 py-3 opacity-70" { (o.email.clone().unwrap_or_else(|| "—".into())) }
                                    td class="px-4 py-3 text-right tabular-nums" { (o.currency) " " (o.total) }
                                    td class="px-4 py-3" {
                                        span class="rounded-full bg-black/5 px-2 py-0.5 text-xs uppercase tracking-widest" {
                                            (o.financial_status.clone().unwrap_or_else(|| "—".into()))
                                        }
                                    }
                                    td class="px-4 py-3" {
                                        @if let Some(url) = &o.tracking_url {
                                            a href=(url) target="_blank" class="text-[color:var(--hot)] underline" { (o.fulfilment_label()) " →" }
                                        } @else {
                                            span class={ @if o.fulfillment_status.as_deref() == Some("fulfilled") { "text-green-600" } @else { "text-amber-600" } } {
                                                (o.fulfilment_label())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    Ok(Html(shell("Orders — RBE", "RBE orders and fulfilment.", Nav::None, body).into_string()))
}

fn stat(label: &str, value: &str) -> maud::Markup {
    html! {
        div class="rounded-lg border border-ink/10 bg-white p-4" {
            div class="font-display text-2xl text-[color:var(--hot)]" { (value) }
            div class="text-xs uppercase tracking-widest opacity-60" { (label) }
        }
    }
}
