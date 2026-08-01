//! Staff-facing pages: the owner dashboard (`/dashboard`) plus the admin-only
//! users/access area (`/admin/team`) for managing staff and customer accounts.

use std::collections::HashMap;

use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde::Deserialize;

use crate::auth::{self, AdminUser, StaffUser};
use crate::db;
use crate::error::AppResult;
use crate::models::Customer;
use crate::state::AppState;

use super::layout::{Nav, shell};

/// GET /dashboard — the owner's business view (admins can see it too).
pub async fn dashboard_owner(
    user: StaffUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let analytics = super::analytics::dashboard_section(&state).await?;
    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-16" {
            div class="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_minmax(18rem,0.9fr)]" {
                section class="rounded-[2rem] border border-ink/10 bg-[linear-gradient(135deg,rgba(255,0,130,0.08),rgba(255,255,255,0.94),rgba(35,10,18,0.08))] p-8 shadow-[0_24px_80px_rgba(35,10,18,0.08)]" {
                    div class="text-xs uppercase tracking-[0.35em] text-[color:var(--hot)]" { "Owner dashboard" }
                    h1 class="mt-3 font-display text-5xl leading-none md:text-7xl" { "Run RBE with clarity." }
                    p class="mt-4 max-w-2xl text-sm leading-7 opacity-70" {
                        "This is your operating room for the brand: watch traffic, keep content moving, manage orders, and jump into deeper website controls when you need them."
                    }
                    div class="mt-6 flex flex-wrap gap-3 text-xs uppercase tracking-widest" {
                        span class="rounded-full border border-ink/15 bg-white/80 px-4 py-2" { (user.email) }
                        span class="rounded-full border border-ink/15 bg-white/80 px-4 py-2" { (user.role) " access" }
                    }
                    div class="mt-8 flex flex-wrap gap-3" {
                        a href="/dashboard/orders" class="inline-flex items-center justify-center rounded-full bg-[color:var(--hot)] px-5 py-3 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "View orders" }
                        a href="/dashboard/posts" class="inline-flex items-center justify-center rounded-full border border-ink/15 bg-white px-5 py-3 text-sm font-semibold uppercase tracking-widest hover:border-[color:var(--hot)] hover:text-[color:var(--hot)]" { "Manage journal" }
                        @if user.is_admin() {
                            a href="/admin" class="inline-flex items-center justify-center rounded-full border border-ink/15 bg-white px-5 py-3 text-sm font-semibold uppercase tracking-widest hover:border-[color:var(--hot)] hover:text-[color:var(--hot)]" { "Open admin" }
                        }
                    }
                }
                aside class="rounded-[2rem] border border-ink/10 bg-ink p-7 text-[color:var(--cream)] shadow-[0_24px_80px_rgba(35,10,18,0.18)]" {
                    div class="text-xs uppercase tracking-[0.35em] text-white/55" { "Quick actions" }
                    div class="mt-5 space-y-3" {
                        (dark_card("Orders & fulfilment", "Track incoming Shopify orders and shipping updates.", "/dashboard/orders"))
                        (dark_card("Journal", "Publish posts and upload fresh cover images.", "/dashboard/posts"))
                        @if user.is_admin() {
                            (dark_card("Users & access", "Manage staff logins and customer accounts.", "/admin/team"))
                        }
                    }
                    a href="/auth/logout" class="mt-6 inline-flex items-center text-xs uppercase tracking-[0.3em] text-white/55 hover:text-white" { "Sign out" }
                }
            }

            div class="mt-8" { (analytics) }

            div class="mt-10 grid gap-4 lg:grid-cols-3" {
                (light_card("Orders & fulfilment", "Every Shopify order and its fulfilment status, in one place.", "/dashboard/orders"))
                (light_card("Journal", "Write, edit, and publish posts to the RBE journal.", "/dashboard/posts"))
                @if user.is_admin() {
                    (light_card("Website admin", "Manage storefront tools, Shopify visibility, users, and settings.", "/admin"))
                }
            }
        }
    };
    Ok(Html(
        shell("Dashboard - RBE", "RBE owner dashboard.", Nav::None, body).into_string(),
    ))
}

fn light_card(title: &str, desc: &str, href: &str) -> Markup {
    html! {
        a href=(href) class="block rounded-[1.5rem] border border-ink/10 bg-white p-6 transition hover:-translate-y-0.5 hover:border-[color:var(--hot)] hover:shadow-[0_24px_60px_rgba(255,0,130,0.12)]" {
            div class="font-display text-3xl leading-none" { (title) }
            p class="mt-2 text-sm opacity-70" { (desc) }
        }
    }
}

fn dark_card(title: &str, desc: &str, href: &str) -> Markup {
    html! {
        a href=(href) class="block rounded-[1.25rem] border border-white/10 bg-white/5 p-4 transition hover:border-[color:var(--hot)] hover:bg-white/10" {
            div class="font-display text-2xl leading-none" { (title) }
            p class="mt-2 text-sm text-white/70" { (desc) }
        }
    }
}

#[derive(serde::Deserialize, Clone)]
struct StaffRow {
    email: String,
    role: String,
}

#[derive(serde::Deserialize)]
struct OrderEmailRow {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Clone)]
struct CustomerSummary {
    email: String,
    name: Option<String>,
    orders: usize,
}

struct UserData {
    staff: Vec<StaffRow>,
    customers: Vec<CustomerSummary>,
}

#[derive(Default)]
struct UserStats {
    staff_total: usize,
    owners: usize,
    customers: usize,
}

/// GET /admin/team — manage staff and customer access.
pub async fn team_page(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> AppResult<Html<String>> {
    let data = load_user_data(&state).await?;
    Ok(Html(
        render_team(&data.staff, &data.customers, None).into_string(),
    ))
}

async fn load_user_data(state: &AppState) -> AppResult<UserData> {
    let staff: Vec<StaffRow> = state
        .db()
        .query("SELECT email, role FROM staff ORDER BY role, email")
        .await?
        .take(0)?;
    let customers: Vec<Customer> = state
        .db()
        .query("SELECT * FROM customer ORDER BY email")
        .await?
        .take(0)?;
    let order_rows: Vec<OrderEmailRow> = state.db().query("SELECT email FROM order").await?.take(0)?;

    let mut order_counts: HashMap<String, usize> = HashMap::new();
    for row in order_rows {
        if let Some(email) = row.email.filter(|email| !email.trim().is_empty()) {
            *order_counts.entry(email.to_lowercase()).or_insert(0) += 1;
        }
    }

    let customers = customers
        .into_iter()
        .map(|customer| CustomerSummary {
            orders: order_counts
                .get(&customer.email.to_lowercase())
                .copied()
                .unwrap_or(0),
            email: customer.email,
            name: customer.name,
        })
        .collect();

    Ok(UserData { staff, customers })
}

fn user_stats(staff: &[StaffRow], customers: &[CustomerSummary]) -> UserStats {
    UserStats {
        staff_total: staff.len(),
        owners: staff.iter().filter(|row| row.role == auth::ROLE_OWNER).count(),
        customers: customers.len(),
    }
}

fn render_team(staff: &[StaffRow], customers: &[CustomerSummary], error: Option<&str>) -> Markup {
    let stats = user_stats(staff, customers);
    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-16" {
            div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between" {
                div {
                    div class="text-xs uppercase tracking-[0.35em] text-[color:var(--hot)]" { "Users & access" }
                    h1 class="mt-2 font-display text-5xl md:text-6xl" { "Manage who gets in." }
                    p class="mt-3 max-w-3xl text-sm leading-7 opacity-70" {
                        "Staff accounts control the dashboard and website tools. Customer accounts belong to shoppers and can be reset or removed here if someone gets stuck."
                    }
                }
                a href="/admin" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "<- Admin" }
            }

            div class="mt-8 grid gap-4 sm:grid-cols-3" {
                (summary_tile("Staff accounts", &stats.staff_total.to_string()))
                (summary_tile("Owners", &stats.owners.to_string()))
                (summary_tile("Customers", &stats.customers.to_string()))
            }

            @if let Some(err) = error {
                div class="mt-6 rounded-2xl border border-red-200 bg-red-50 px-5 py-4 text-sm text-red-700" { (err) }
            }

            div class="mt-10 grid gap-6 xl:grid-cols-[minmax(0,1.55fr)_minmax(20rem,0.95fr)]" {
                section class="space-y-6" {
                    div class="rounded-[1.75rem] border border-ink/10 bg-white p-6 shadow-[0_20px_60px_rgba(35,10,18,0.06)]" {
                        div class="flex items-center justify-between gap-4" {
                            div {
                                h2 class="font-display text-4xl leading-none" { "Staff" }
                                p class="mt-2 text-sm opacity-65" { "Admins manage everything. Owners land on the business dashboard only." }
                            }
                            span class="rounded-full bg-black/5 px-3 py-1 text-xs uppercase tracking-widest opacity-70" { (format!("{} total", stats.staff_total)) }
                        }
                        div class="mt-6 space-y-4" {
                            @for s in staff {
                                div class="rounded-2xl border border-ink/10 bg-[color:var(--cream)]/55 p-4" {
                                    div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between" {
                                        div {
                                            div class="text-lg font-semibold" { (&s.email) }
                                            div class="mt-2 flex flex-wrap gap-2" {
                                                span class={ @if s.role == auth::ROLE_ADMIN { "rounded-full bg-ink px-3 py-1 text-[10px] uppercase tracking-[0.3em] text-white" } @else { "rounded-full bg-[color:var(--blush)] px-3 py-1 text-[10px] uppercase tracking-[0.3em]" } } { (&s.role) }
                                                @if s.role == auth::ROLE_ADMIN {
                                                    span class="rounded-full border border-ink/15 bg-white px-3 py-1 text-[10px] uppercase tracking-[0.3em] opacity-60" { "Protected" }
                                                }
                                            }
                                        }
                                        div class="w-full max-w-xl space-y-3" {
                                            form method="post" action="/admin/team/staff-password" class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" {
                                                input type="hidden" name="email" value=(&s.email);
                                                input name="password" type="password" required minlength="8" placeholder="New password (min 8 chars)"
                                                    class="w-full rounded-full border border-ink/15 bg-white px-4 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                                                button type="submit"
                                                    class="rounded-full bg-[color:var(--hot)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.25em] text-white hover:bg-[color:var(--crimson)]" {
                                                    "Reset password"
                                                }
                                            }
                                            @if s.role != auth::ROLE_ADMIN {
                                                form method="post" action="/admin/team/staff-delete" {
                                                    input type="hidden" name="email" value=(&s.email);
                                                    button type="submit"
                                                        class="rounded-full border border-red-200 px-4 py-2 text-xs font-semibold uppercase tracking-[0.25em] text-red-600 hover:border-red-400 hover:text-red-700" {
                                                        "Delete owner"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div class="rounded-[1.75rem] border border-ink/10 bg-white p-6 shadow-[0_20px_60px_rgba(35,10,18,0.06)]" {
                        div class="flex items-center justify-between gap-4" {
                            div {
                                h2 class="font-display text-4xl leading-none" { "Customers" }
                                p class="mt-2 text-sm opacity-65" { "Reset passwords or remove accounts if a shopper gets locked out." }
                            }
                            span class="rounded-full bg-black/5 px-3 py-1 text-xs uppercase tracking-widest opacity-70" { (format!("{} accounts", stats.customers)) }
                        }
                        @if customers.is_empty() {
                            div class="mt-6 rounded-2xl border border-dashed border-ink/15 bg-[color:var(--cream)]/55 p-8 text-center text-sm opacity-65" {
                                "No customer accounts have been created yet."
                            }
                        } @else {
                            div class="mt-6 space-y-4" {
                                @for customer in customers {
                                    div class="rounded-2xl border border-ink/10 bg-[color:var(--cream)]/55 p-4" {
                                        div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between" {
                                            div {
                                                div class="text-lg font-semibold" {
                                                    @if let Some(name) = customer.name.as_deref().filter(|name| !name.trim().is_empty()) {
                                                        (name)
                                                    } @else {
                                                        "Customer"
                                                    }
                                                }
                                                div class="mt-1 text-sm opacity-70" { (&customer.email) }
                                                div class="mt-2 flex flex-wrap gap-2" {
                                                    span class="rounded-full bg-white px-3 py-1 text-[10px] uppercase tracking-[0.3em] opacity-70" { "Shopper" }
                                                    span class="rounded-full bg-white px-3 py-1 text-[10px] uppercase tracking-[0.3em] opacity-70" { (format!("{} orders", customer.orders)) }
                                                }
                                            }
                                            div class="w-full max-w-xl space-y-3" {
                                                form method="post" action="/admin/team/customer-password" class="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" {
                                                    input type="hidden" name="email" value=(&customer.email);
                                                    input name="password" type="password" required minlength="8" placeholder="New password (min 8 chars)"
                                                        class="w-full rounded-full border border-ink/15 bg-white px-4 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                                                    button type="submit"
                                                        class="rounded-full bg-[color:var(--hot)] px-4 py-2 text-xs font-semibold uppercase tracking-[0.25em] text-white hover:bg-[color:var(--crimson)]" {
                                                        "Reset password"
                                                    }
                                                }
                                                form method="post" action="/admin/team/customer-delete" {
                                                    input type="hidden" name="email" value=(&customer.email);
                                                    button type="submit"
                                                        class="rounded-full border border-red-200 px-4 py-2 text-xs font-semibold uppercase tracking-[0.25em] text-red-600 hover:border-red-400 hover:text-red-700" {
                                                        "Delete customer"
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

                aside class="rounded-[1.75rem] border border-ink/10 bg-ink p-6 text-[color:var(--cream)] shadow-[0_20px_60px_rgba(35,10,18,0.18)]" {
                    div class="text-xs uppercase tracking-[0.35em] text-white/55" { "Add owner" }
                    h2 class="mt-3 font-display text-4xl leading-none" { "Invite a new operator." }
                    p class="mt-3 text-sm leading-7 text-white/70" {
                        "Owners can access the business dashboard, orders, and journal tools. They do not get the full website-admin controls."
                    }
                    form method="post" action="/admin/team" class="mt-6 space-y-3" {
                        input name="email" type="email" required placeholder="owner@email.com"
                            class="w-full rounded-full border border-white/10 bg-white/10 px-4 py-3 text-sm text-white outline-none placeholder:text-white/40 focus:border-[color:var(--hot)]";
                        input name="password" type="password" required minlength="8" placeholder="Temporary password (min 8 chars)"
                            class="w-full rounded-full border border-white/10 bg-white/10 px-4 py-3 text-sm text-white outline-none placeholder:text-white/40 focus:border-[color:var(--hot)]";
                        button type="submit"
                            class="w-full rounded-full bg-[color:var(--hot)] px-5 py-3 text-sm font-semibold uppercase tracking-[0.25em] text-white hover:bg-[color:var(--crimson)]" {
                            "Create owner"
                        }
                    }
                    div class="mt-6 rounded-2xl border border-white/10 bg-white/5 p-4 text-sm text-white/70" {
                        "Tip: if someone forgets a password later, use the reset controls on the left instead of creating a duplicate account."
                    }
                    a href="/dashboard" class="mt-6 inline-flex items-center text-xs uppercase tracking-[0.3em] text-white/55 hover:text-white" { "Open dashboard" }
                }
            }
        }
    };
    shell("Users - RBE Admin", "Manage RBE staff and customer access.", Nav::None, body)
}

fn summary_tile(label: &str, value: &str) -> Markup {
    html! {
        div class="rounded-[1.4rem] border border-ink/10 bg-white p-5 shadow-[0_14px_40px_rgba(35,10,18,0.05)]" {
            div class="font-display text-4xl leading-none text-[color:var(--hot)]" { (value) }
            div class="mt-2 text-xs uppercase tracking-[0.3em] opacity-60" { (label) }
        }
    }
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
        return re_render_error(
            &state,
            "Enter a valid email and a password of at least 8 characters.",
        )
        .await;
    }
    let hash = match auth::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match db::create_staff(state.db(), &email, &hash, auth::ROLE_OWNER).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(
            &state,
            "Could not create that account. The email may already be in use.",
        )
        .await,
    }
}

#[derive(Deserialize)]
pub struct PasswordUpdateForm {
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub struct DeleteAccountForm {
    email: String,
}

pub async fn staff_password_update(
    _admin: AdminUser,
    State(state): State<AppState>,
    Form(form): Form<PasswordUpdateForm>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || form.password.len() < 8 {
        return re_render_error(
            &state,
            "Use a valid email and a password with at least 8 characters.",
        )
        .await;
    }
    let hash = match auth::hash_password(&form.password) {
        Ok(hash) => hash,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match db::update_staff_password(state.db(), &email, &hash).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(&state, "Could not update that staff password.").await,
    }
}

pub async fn staff_delete(
    _admin: AdminUser,
    State(state): State<AppState>,
    Form(form): Form<DeleteAccountForm>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    if email.is_empty() {
        return re_render_error(&state, "Choose a valid owner account to delete.").await;
    }

    let data = match load_user_data(&state).await {
        Ok(data) => data,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some(target) = data.staff.iter().find(|row| row.email == email) else {
        return re_render_error(&state, "That staff account no longer exists.").await;
    };
    if target.role == auth::ROLE_ADMIN {
        return re_render_error(
            &state,
            "The main admin account is protected and cannot be deleted here.",
        )
        .await;
    }

    match db::delete_staff(state.db(), &email).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(&state, "Could not delete that owner account.").await,
    }
}

pub async fn customer_password_update(
    _admin: AdminUser,
    State(state): State<AppState>,
    Form(form): Form<PasswordUpdateForm>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    if email.is_empty() || form.password.len() < 8 {
        return re_render_error(
            &state,
            "Use a valid email and a password with at least 8 characters.",
        )
        .await;
    }
    let hash = match auth::hash_password(&form.password) {
        Ok(hash) => hash,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match db::update_customer_password(state.db(), &email, &hash).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(&state, "Could not update that customer password.").await,
    }
}

pub async fn customer_delete(
    _admin: AdminUser,
    State(state): State<AppState>,
    Form(form): Form<DeleteAccountForm>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    if email.is_empty() {
        return re_render_error(&state, "Choose a valid customer account to delete.").await;
    }

    match db::delete_customer(state.db(), &email).await {
        Ok(_) => Redirect::to("/admin/team").into_response(),
        Err(_) => re_render_error(&state, "Could not delete that customer account.").await,
    }
}

async fn re_render_error(state: &AppState, msg: &str) -> Response {
    let data = load_user_data(state).await.unwrap_or(UserData {
        staff: Vec::new(),
        customers: Vec::new(),
    });
    (
        StatusCode::BAD_REQUEST,
        Html(render_team(&data.staff, &data.customers, Some(msg)).into_string()),
    )
        .into_response()
}
