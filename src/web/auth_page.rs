//! Admin login (email + password). Replaces the reference Supabase/Lovable
//! OAuth flow with a single admin credential issuing a JWT session cookie.

use axum::Form;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, html};
use serde::Deserialize;

use crate::auth;
use crate::state::AppState;

use super::layout::{Nav, shell};

fn login_form(error: Option<&str>) -> Markup {
    html! {
        div class="mx-auto max-w-md px-4 py-16" {
            h1 class="font-display text-4xl" { "Sign in" }
            p class="mt-2 text-sm opacity-60" { "Admin access to the RBE control panel." }
            form method="post" action="/auth" class="mt-6 space-y-4" {
                div {
                    label class="text-sm font-medium" for="email" { "Email" }
                    input id="email" name="email" type="email" required
                        class="mt-1 w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                }
                div {
                    label class="text-sm font-medium" for="password" { "Password" }
                    input id="password" name="password" type="password" required
                        class="mt-1 w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
                }
                @if let Some(err) = error {
                    p class="text-sm text-red-500" { (err) }
                }
                button type="submit"
                    class="w-full rounded-full bg-[color:var(--hot)] px-4 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" {
                    "Sign in"
                }
            }
        }
    }
}

pub async fn login_page() -> Html<String> {
    Html(shell("Sign in — RBE", "Sign in to RBE admin.", Nav::None, login_form(None)).into_string())
}

#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

pub async fn login_submit(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    let Some((email, role)) = auth::authenticate_staff(&state, &form.email, &form.password).await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Html(
                shell(
                    "Sign in — RBE",
                    "Sign in to RBE admin.",
                    Nav::None,
                    login_form(Some("Invalid email or password.")),
                )
                .into_string(),
            ),
        )
            .into_response();
    };

    match auth::issue_token(state.cfg(), &email, &role) {
        Ok(token) => {
            // All staff land on the main dashboard first; admins can step into
            // the deeper website-admin tools from there.
            let dest = "/dashboard";
            (
                [(header::SET_COOKIE, auth::session_cookie(auth::STAFF_COOKIE, &token))],
                Redirect::to(dest),
            )
                .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn logout() -> Response {
    (
        [(header::SET_COOKIE, auth::clear_cookie(auth::STAFF_COOKIE))],
        Redirect::to("/"),
    )
        .into_response()
}
