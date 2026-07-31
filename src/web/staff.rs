//! Staff-facing pages: the owner business dashboard (`/dashboard`) and the
//! admin-only Team screen (`/admin/team`) for creating owner accounts.
//!
//! The dashboard is a shell for now; analytics, orders and blog shortcuts land
//! in later deliverables.

use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde::Deserialize;

use crate::auth::{self, AdminUser, StaffUser};
use crate::db;
use crate::error::AppResult;
use crate::state::AppState;

use super::layout::{Nav, shell};

/// GET /dashboard — the owner's business view (admins can see it too).
pub async fn dashboard_owner(user: StaffUser, State(state): State<AppState>) -> AppResult<Html<String>> {
    let analytics = super::analytics::dashboard_section(&state).await?;
    let body = html! {
        div class="mx-auto max-w-4xl px-4 py-16" {
            div class="flex items-center justify-between" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Owner dashboard" }
                    h1 class="mt-1 font-display text-5xl" { "Welcome back" }
                    p class="mt-2 text-sm opacity-60" { "Signed in as " (user.email) " · " (user.role) }
                }
                a href="/auth/logout" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "Sign out" }
            }

            div class="mt-8" { (analytics) }

            div class="mt-10 grid gap-4 sm:grid-cols-2" {
                (card("Sales & analytics", "Revenue, orders and traffic — coming in this build.", "#"))
                (card("Orders & fulfilment", "Every Shopify order and its Printify tracking, in one place.", "/dashboard/orders"))
                (card("Journal", "Write and publish posts to the RBE journal.", "/dashboard/posts"))
                @if user.is_admin() {
                    (card("Admin control panel", "Products, Printify sync, team and settings.", "/admin"))
                }
            }
        }
    };
    Ok(Html(shell("Dashboard — RBE", "RBE owner dashboard.", Nav::None, body).into_string()))
}

fn card(title: &str, desc: &str, href: &str) -> Markup {
    html! {
        a href=(href) class="block rounded-lg border border-ink/10 bg-white p-6 transition hover:border-[color:var(--hot)] hover:shadow-lg hover:shadow-[color:var(--hot)]/10" {
            div class="font-display text-2xl" { (title) }
            p class="mt-2 text-sm opacity-70" { (desc) }
        }
    }
}

#[derive(serde::Deserialize)]
struct StaffRow {
    email: String,
    role: String,
}

/// GET /admin/team — list staff and a form to add an owner.
pub async fn team_page(_admin: AdminUser, State(state): State<AppState>) -> AppResult<Html<String>> {
    let staff: Vec<StaffRow> = state
        .db()
        .query("SELECT email, role FROM staff ORDER BY role, email")
        .await?
        .take(0)?;
    Ok(Html(render_team(&state, &staff, None).into_string()))
}

fn render_team(_state: &AppState, staff: &[StaffRow], error: Option<&str>) -> Markup {
    let body = html! {
        div class="mx-auto max-w-2xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Team" }
                a href="/admin" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "← Admin" }
            }
            p class="mt-2 text-sm opacity-60" { "Admins manage everything; owners get the business dashboard." }

            ul class="mt-8 divide-y rounded-lg border border-ink/10 bg-white" {
                @for s in staff {
                    li class="flex items-center justify-between px-4 py-3" {
                        span class="font-medium" { (s.email) }
                        span class="rounded-full bg-[color:var(--blush)] px-2 py-0.5 text-xs uppercase tracking-widest" { (s.role) }
                    }
                }
            }

            h2 class="mt-10 font-semibold" { "Add an owner" }
            form method="post" action="/admin/team" class="mt-3 space-y-3" {
                input name="email" type="email" required placeholder="owner@email.com"
                    class="w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                input name="password" type="password" required minlength="8" placeholder="Temporary password (min 8 chars)"
                    class="w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                @if let Some(err) = error {
                    p class="text-sm text-red-500" { (err) }
                }
                button type="submit"
                    class="rounded-full bg-[color:var(--hot)] px-5 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" {
                    "Create owner"
                }
            }
        }
    };
    shell("Team — RBE Admin", "Manage RBE staff.", Nav::None, body)
}

#[derive(Deserialize)]
pub struct NewStaff {
    email: String,
    password: String,
}

/// POST /admin/team — create an owner account.
pub async fn team_create(
    _admin: AdminUser,
    State(state): State<AppState>,
    Form(form): Form<NewStaff>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || form.password.len() < 8 {
        return re_render_error(&state, "Enter a valid email and a password of at least 8 characters.").await;
    }
    let hash = match auth::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match db::create_staff(state.db(), &email, &hash, auth::ROLE_OWNER).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(&state, "Could not create that account — the email may already be in use.").await,
    }
}

async fn re_render_error(state: &AppState, msg: &str) -> Response {
    let staff: Vec<StaffRow> = state
        .db()
        .query("SELECT email, role FROM staff ORDER BY role, email")
        .await
        .and_then(|mut r| r.take(0))
        .unwrap_or_default();
    (
        StatusCode::BAD_REQUEST,
        Html(render_team(state, &staff, Some(msg)).into_string()),
    )
        .into_response()
}
