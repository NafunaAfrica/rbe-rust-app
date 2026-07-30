//! Admin authentication: email + password login issuing a signed JWT session
//! cookie, plus an extractor that guards `/admin` routes.
//!
//! Scope (per the rebuild decision): a single admin account, configured via
//! env. `argon2` is wired in for when we graduate to stored user records with
//! hashed passwords; today we constant-time compare against the configured
//! credential.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::state::AppState;

pub const COOKIE_NAME: &str = "rbe_session";
const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 7; // 7 days

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the admin email.
    pub sub: String,
    pub exp: i64,
}

/// Verify a login attempt against the configured admin credential.
pub fn verify_credentials(cfg: &crate::config::Config, email: &str, password: &str) -> bool {
    let email_ok = email.trim().eq_ignore_ascii_case(&cfg.admin_email);
    // constant-time password comparison
    let pw_ok: bool = password
        .as_bytes()
        .ct_eq(cfg.admin_password.as_bytes())
        .into();
    email_ok & pw_ok
}

/// Mint a signed session token for the given admin email.
pub fn issue_token(cfg: &crate::config::Config, email: &str) -> anyhow::Result<String> {
    let exp = (time::OffsetDateTime::now_utc().unix_timestamp()) + SESSION_TTL_SECS;
    let claims = Claims {
        sub: email.to_string(),
        exp,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )?;
    Ok(token)
}

fn decode_token(cfg: &crate::config::Config, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

fn cookie_from_headers(parts: &Parts, name: &str) -> Option<String> {
    let raw = parts.headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == name)
        .map(|(_, v)| v.trim().to_string())
}

/// Build the `Set-Cookie` header value for a session token.
pub fn session_cookie(token: &str) -> String {
    format!(
        "{COOKIE_NAME}={token}; HttpOnly; Path=/; SameSite=Lax; Max-Age={SESSION_TTL_SECS}"
    )
}

/// Build the `Set-Cookie` header value that clears the session.
pub fn clear_cookie() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0")
}

/// Request extractor that authenticates the admin. On failure it redirects to
/// the login page, so guarded handlers can assume a valid admin.
pub struct AdminUser {
    #[allow(dead_code)]
    pub email: String,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = cookie_from_headers(parts, COOKIE_NAME)
            .ok_or_else(|| Redirect::to("/auth").into_response())?;
        let claims = decode_token(state.cfg(), &token)
            .ok_or_else(|| Redirect::to("/auth").into_response())?;
        Ok(AdminUser { email: claims.sub })
    }
}
