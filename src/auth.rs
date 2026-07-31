//! Authentication for RBE's three roles, backed by SurrealDB.
//!
//! Two realms, each a signed-JWT cookie:
//! - **staff** (`rbe_staff`): roles `admin` (Nafuna / full control) and `owner`
//!   (the store owner's business view).
//! - **customer** (`rbe_customer`): shoppers.
//!
//! Passwords are argon2 hashes; the JWT carries the role so route guards don't
//! hit the database on every request.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::models::{Customer, Staff};
use crate::state::AppState;

pub const STAFF_COOKIE: &str = "rbe_staff";
pub const CUSTOMER_COOKIE: &str = "rbe_customer";
const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 7; // 7 days

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_OWNER: &str = "owner";
pub const ROLE_CUSTOMER: &str = "customer";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the account email.
    pub sub: String,
    /// "admin" | "owner" | "customer"
    pub role: String,
    pub exp: i64,
}

// ---------------------------------------------------------------------------
// Password hashing (argon2)
// ---------------------------------------------------------------------------

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hash failed: {e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(pw.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

pub fn issue_token(cfg: &Config, email: &str, role: &str) -> anyhow::Result<String> {
    let exp = time::OffsetDateTime::now_utc().unix_timestamp() + SESSION_TTL_SECS;
    let claims = Claims {
        sub: email.to_string(),
        role: role.to_string(),
        exp,
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )?)
}

fn decode_token(cfg: &Config, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

// ---------------------------------------------------------------------------
// Authenticate against the database
// ---------------------------------------------------------------------------

// NOTE: we look accounts up with a plain `SELECT *` and match the email in Rust
// rather than `WHERE email = $e`. On the production volume the unique email
// indexes were left in a bad state by an earlier SurrealDB version, so
// index-backed `WHERE` lookups return nothing (writes / uniqueness still work,
// which is why registration succeeds but login couldn't find the account). Full
// scans are fine at this scale and don't touch the index. The UNIQUE index is
// kept purely to enforce no-duplicate-signups on the write path.

/// Returns `(email, role)` on success.
pub async fn authenticate_staff(state: &AppState, email: &str, pw: &str) -> Option<(String, String)> {
    let email = email.trim().to_lowercase();
    let all: Vec<Staff> = state
        .db()
        .query("SELECT * FROM staff")
        .await
        .ok()?
        .take(0)
        .ok()?;
    let s = all.into_iter().find(|s| s.email == email)?;
    verify_password(pw, &s.password_hash).then_some((s.email, s.role))
}

/// Returns the customer email on success.
pub async fn authenticate_customer(state: &AppState, email: &str, pw: &str) -> Option<String> {
    let email = email.trim().to_lowercase();
    let all: Vec<Customer> = state
        .db()
        .query("SELECT * FROM customer")
        .await
        .ok()?
        .take(0)
        .ok()?;
    let c = all.into_iter().find(|c| c.email == email)?;
    verify_password(pw, &c.password_hash).then_some(c.email)
}

// ---------------------------------------------------------------------------
// Cookies
// ---------------------------------------------------------------------------

fn cookie_from_headers(parts: &Parts, name: &str) -> Option<String> {
    let raw = parts.headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == name)
        .map(|(_, v)| v.trim().to_string())
}

pub fn session_cookie(name: &str, token: &str) -> String {
    format!("{name}={token}; HttpOnly; Path=/; SameSite=Lax; Max-Age={SESSION_TTL_SECS}")
}

pub fn clear_cookie(name: &str) -> String {
    format!("{name}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0")
}

/// Read + verify the claims from a given cookie, if present and valid.
pub fn claims_from(parts: &Parts, cfg: &Config, cookie: &str) -> Option<Claims> {
    cookie_from_headers(parts, cookie).and_then(|t| decode_token(cfg, &t))
}

/// Convenience for JSON API handlers that hold a `HeaderMap` rather than
/// request `Parts`: returns the signed-in shopper's email, if any.
pub fn customer_email(headers: &axum::http::HeaderMap, cfg: &Config) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    let token = raw
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == CUSTOMER_COOKIE)
        .map(|(_, v)| v.trim().to_string())?;
    let claims = decode_token(cfg, &token)?;
    (claims.role == ROLE_CUSTOMER).then_some(claims.sub)
}

// ---------------------------------------------------------------------------
// Extractors
// ---------------------------------------------------------------------------

/// Admin only (role = admin). Redirects to the staff login on failure.
pub struct AdminUser {
    #[allow(dead_code)]
    pub email: String,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        match claims_from(parts, state.cfg(), STAFF_COOKIE) {
            Some(c) if c.role == ROLE_ADMIN => Ok(AdminUser { email: c.sub }),
            _ => Err(Redirect::to("/auth").into_response()),
        }
    }
}

/// Any staff member (admin or owner). Used for the business dashboard.
pub struct StaffUser {
    pub email: String,
    pub role: String,
}

impl StaffUser {
    pub fn is_admin(&self) -> bool {
        self.role == ROLE_ADMIN
    }
}

impl FromRequestParts<AppState> for StaffUser {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        match claims_from(parts, state.cfg(), STAFF_COOKIE) {
            Some(c) if c.role == ROLE_ADMIN || c.role == ROLE_OWNER => Ok(StaffUser {
                email: c.sub,
                role: c.role,
            }),
            _ => Err(Redirect::to("/auth").into_response()),
        }
    }
}

/// A signed-in shopper.
pub struct CustomerUser {
    pub email: String,
}

impl FromRequestParts<AppState> for CustomerUser {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        match claims_from(parts, state.cfg(), CUSTOMER_COOKIE) {
            Some(c) if c.role == ROLE_CUSTOMER => Ok(CustomerUser { email: c.sub }),
            _ => Err(Redirect::to("/account/login").into_response()),
        }
    }
}
