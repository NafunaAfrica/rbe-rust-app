//! Customer accounts: register, login, and an account page with order history
//! (joined from ingested Shopify orders by email). Checkout still happens on
//! Shopify — these accounts are for the on-domain relationship + tracking.

use axum::Form;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde::Deserialize;

use crate::auth::{self, CustomerUser};
use crate::db;
use crate::error::AppResult;
use crate::models::Order;
use crate::state::AppState;

use super::layout::{Nav, shell};

const FIELD: &str = "mt-1 w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";

/// `?next=` carried through login/register so the checkout gate can return the
/// shopper to their bag. Only same-site paths are honoured.
#[derive(Deserialize)]
pub struct NextQuery {
    #[serde(default)]
    next: String,
}

fn safe_next(next: &str) -> Option<String> {
    (next.starts_with('/') && !next.starts_with("//")).then(|| next.to_string())
}

/// Build a `?next=…` suffix for a link, or empty string when there's nothing to
/// carry. Percent-encodes the handful of chars that would break an href query
/// value (our `next` is always a controlled same-site path like `/shop#cart`).
fn next_qs(next: &str) -> String {
    let Some(n) = safe_next(next) else {
        return String::new();
    };
    let enc: String = n
        .chars()
        .map(|c| match c {
            '#' => "%23".into(),
            '&' => "%26".into(),
            '?' => "%3F".into(),
            ' ' => "%20".into(),
            other => other.to_string(),
        })
        .collect();
    format!("?next={enc}")
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

fn register_form(error: Option<&str>, next: &str) -> Markup {
    let body = html! {
        div class="mx-auto max-w-md px-4 py-16" {
            h1 class="font-display text-4xl" { "Create account" }
            p class="mt-2 text-sm opacity-60" { "Track your orders and join the club." }
            form method="post" action="/account/register" class="mt-6 space-y-4" {
                input type="hidden" name="next" value=(next);
                div { label class="text-sm font-medium" { "Name" } input name="name" class=(FIELD); }
                div { label class="text-sm font-medium" { "Email" } input name="email" type="email" required class=(FIELD); }
                div { label class="text-sm font-medium" { "Password" } input name="password" type="password" required minlength="8" class=(FIELD); }
                @if let Some(e) = error { p class="text-sm text-red-500" { (e) } }
                button type="submit" class="w-full rounded-full bg-[color:var(--hot)] px-4 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "Create account" }
            }
            p class="mt-4 text-center text-sm opacity-70" {
                "Already have an account? " a href=(format!("/account/login{}", next_qs(next))) class="text-[color:var(--hot)] underline" { "Sign in" }
            }
        }
    };
    shell("Create account — RBE", "Create your RBE account.", Nav::None, body)
}

pub async fn register_page(Query(q): Query<NextQuery>) -> Html<String> {
    Html(register_form(None, &q.next).into_string())
}

#[derive(Deserialize)]
pub struct RegisterForm {
    email: String,
    password: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    next: String,
}

pub async fn register_submit(State(state): State<AppState>, Form(f): Form<RegisterForm>) -> Response {
    let email = f.email.trim().to_lowercase();
    if email.is_empty() || f.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, Html(register_form(Some("Enter a valid email and a password of at least 8 characters."), &f.next).into_string())).into_response();
    }
    let hash = match auth::hash_password(&f.password) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let name = (!f.name.trim().is_empty()).then(|| f.name.trim());
    if db::create_customer(state.db(), &email, &hash, name).await.is_err() {
        return (StatusCode::CONFLICT, Html(register_form(Some("That email is already registered. Try signing in."), &f.next).into_string())).into_response();
    }
    issue_and_redirect(state.cfg(), &email, &f.next)
}

// ---------------------------------------------------------------------------
// Login / logout
// ---------------------------------------------------------------------------

fn login_form(error: Option<&str>, next: &str) -> Markup {
    let body = html! {
        div class="mx-auto max-w-md px-4 py-16" {
            h1 class="font-display text-4xl" { "Sign in" }
            @if !next.is_empty() {
                p class="mt-2 text-sm opacity-60" { "Sign in to finish checking out." }
            }
            form method="post" action="/account/login" class="mt-6 space-y-4" {
                input type="hidden" name="next" value=(next);
                div { label class="text-sm font-medium" { "Email" } input name="email" type="email" required class=(FIELD); }
                div { label class="text-sm font-medium" { "Password" } input name="password" type="password" required class=(FIELD); }
                @if let Some(e) = error { p class="text-sm text-red-500" { (e) } }
                button type="submit" class="w-full rounded-full bg-[color:var(--hot)] px-4 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "Sign in" }
            }
            p class="mt-4 text-center text-sm opacity-70" {
                "New here? " a href=(format!("/account/register{}", next_qs(next))) class="text-[color:var(--hot)] underline" { "Create an account" }
            }
        }
    };
    shell("Sign in — RBE", "Sign in to your RBE account.", Nav::None, body)
}

pub async fn login_page(Query(q): Query<NextQuery>) -> Html<String> {
    Html(login_form(None, &q.next).into_string())
}

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
    #[serde(default)]
    next: String,
}

pub async fn login_submit(State(state): State<AppState>, Form(f): Form<LoginForm>) -> Response {
    match auth::authenticate_customer(&state, &f.email, &f.password).await {
        Some(email) => issue_and_redirect(state.cfg(), &email, &f.next),
        None => (StatusCode::UNAUTHORIZED, Html(login_form(Some("Invalid email or password."), &f.next).into_string())).into_response(),
    }
}

pub async fn logout() -> Response {
    (
        [(header::SET_COOKIE, auth::clear_cookie(auth::CUSTOMER_COOKIE))],
        Redirect::to("/"),
    )
        .into_response()
}

fn issue_and_redirect(cfg: &crate::config::Config, email: &str, next: &str) -> Response {
    let dest = safe_next(next).unwrap_or_else(|| "/account".to_string());
    match auth::issue_token(cfg, email, auth::ROLE_CUSTOMER) {
        Ok(token) => (
            [(header::SET_COOKIE, auth::session_cookie(auth::CUSTOMER_COOKIE, &token))],
            Redirect::to(&dest),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Account page
// ---------------------------------------------------------------------------

pub async fn account_page(user: CustomerUser, State(state): State<AppState>) -> AppResult<Html<String>> {
    let orders: Vec<Order> = state
        .db()
        .query("SELECT * FROM order WHERE email = $e ORDER BY created_at DESC")
        .bind(("e", user.email.clone()))
        .await?
        .take(0)?;

    let body = html! {
        div class="mx-auto max-w-3xl px-4 py-16" {
            div class="flex items-center justify-between" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Your account" }
                    h1 class="mt-1 font-display text-5xl" { "Hey there" }
                    p class="mt-2 text-sm opacity-60" { (user.email) }
                }
                a href="/account/logout" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "Sign out" }
            }

            h2 class="mt-10 font-semibold" { "Your orders" }
            @if orders.is_empty() {
                div class="mt-3 rounded-lg border border-dashed border-ink/20 bg-white/50 p-8 text-center" {
                    p class="text-sm opacity-70" { "No orders yet. " a href="/shop" class="text-[color:var(--hot)] underline" { "Find your slogan →" } }
                }
            } @else {
                ul class="mt-3 divide-y rounded-lg border border-ink/10 bg-white" {
                    @for o in &orders {
                        li class="flex items-center justify-between px-4 py-3" {
                            div {
                                div class="font-medium" { (o.number.clone().unwrap_or_else(|| o.shopify_order_id.clone())) }
                                div class="text-xs opacity-60" { (o.created_date()) " · " (o.currency) " " (o.total) }
                            }
                            div class="text-sm" {
                                @if let Some(url) = &o.tracking_url {
                                    a href=(url) target="_blank" class="text-[color:var(--hot)] underline" { (o.fulfilment_label()) " →" }
                                } @else {
                                    span class={ @if o.fulfillment_status.as_deref() == Some("fulfilled") { "text-green-600" } @else { "text-amber-600" } } { (o.fulfilment_label()) }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    Ok(Html(shell("Account — RBE", "Your RBE account and orders.", Nav::None, body).into_string()))
}
