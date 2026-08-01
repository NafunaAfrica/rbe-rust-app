//! The journal: public list + article pages (Markdown-rendered), and a
//! staff-only editor at `/dashboard/posts`.

use axum::extract::{Multipart, Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;
use std::path::Path as StdPath;

use crate::auth::StaffUser;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::Post;
use crate::state::AppState;

use super::layout::{Nav, shell};

// ---------------------------------------------------------------------------
// Public
// ---------------------------------------------------------------------------

pub async fn journal(State(state): State<AppState>) -> AppResult<Html<String>> {
    let posts: Vec<Post> = state
        .db()
        .query("SELECT * FROM post WHERE status = 'published' ORDER BY published_at DESC")
        .await?
        .take(0)?;

    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-20 md:px-8" {
            div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Dispatches" }
            h1 class="mt-2 font-display text-6xl md:text-8xl" { "JOURNAL" }
            @if posts.is_empty() {
                p class="mt-8 opacity-60" { "No posts yet — check back soon." }
            } @else {
                div class="mt-12 grid gap-6 md:grid-cols-2" {
                    @for p in &posts {
                        a href=(format!("/journal/{}", p.slug)) class="group block rounded-lg border border-ink/10 bg-white p-8 transition hover:border-[color:var(--hot)] hover:shadow-xl hover:shadow-[color:var(--hot)]/10" {
                            div class="flex items-center justify-between text-xs uppercase tracking-widest opacity-60" {
                                span { (p.tag) } span { (p.published_date()) }
                            }
                            h2 class="mt-4 font-serif-display text-3xl leading-snug group-hover:text-[color:var(--hot)]" { (p.title) }
                            @if !p.excerpt.is_empty() {
                                p class="mt-3 text-sm opacity-70" { (p.excerpt) }
                            }
                            div class="mt-6 text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Read →" }
                        }
                    }
                }
            }
        }
    };
    Ok(Html(shell("Journal — RBE", "Essays, playlists, and dispatches from the RBE club.", Nav::Journal, body).into_string()))
}

pub async fn article(State(state): State<AppState>, Path(slug): Path<String>) -> AppResult<Html<String>> {
    let post: Option<Post> = state
        .db()
        .query("SELECT * FROM type::thing('post', $slug)")
        .bind(("slug", slug))
        .await?
        .take(0)?;
    let post = post.filter(|p| p.is_published()).ok_or(AppError::NotFound)?;

    let body = html! {
        article class="mx-auto max-w-2xl px-4 py-16 md:px-8" {
            a href="/journal" class="text-xs uppercase tracking-widest opacity-60 hover:opacity-100" { "← Journal" }
            div class="mt-6 flex items-center gap-3 text-xs uppercase tracking-widest opacity-60" {
                span class="text-[color:var(--hot)]" { (post.tag) } span { "·" } span { (post.published_date()) }
            }
            h1 class="mt-3 font-display text-5xl leading-none md:text-6xl" { (post.title) }
            @if let Some(cover) = &post.cover_url {
                img src=(cover) alt=(post.title) class="mt-8 w-full rounded-lg object-cover";
            }
            div class="mt-8 text-lg leading-relaxed [&_h2]:font-display [&_h2]:text-3xl [&_h2]:mt-10 [&_h2]:mb-2 [&_h3]:font-semibold [&_h3]:text-xl [&_h3]:mt-8 [&_p]:mt-4 [&_a]:text-[color:var(--hot)] [&_a]:underline [&_ul]:mt-4 [&_ul]:list-disc [&_ul]:pl-6 [&_ol]:mt-4 [&_ol]:list-decimal [&_ol]:pl-6 [&_li]:mt-1 [&_strong]:font-semibold [&_em]:italic [&_blockquote]:mt-6 [&_blockquote]:border-l-2 [&_blockquote]:border-[color:var(--hot)] [&_blockquote]:pl-4 [&_blockquote]:italic [&_blockquote]:opacity-80" {
                (PreEscaped(md_to_html(&post.body_md)))
            }
        }
    };
    Ok(Html(shell(&format!("{} — RBE Journal", post.title), &post.excerpt, Nav::Journal, body).into_string()))
}

// ---------------------------------------------------------------------------
// Editor (staff)
// ---------------------------------------------------------------------------

pub async fn posts_list(_user: StaffUser, State(state): State<AppState>) -> AppResult<Html<String>> {
    let posts: Vec<Post> = state
        .db()
        .query("SELECT * FROM post ORDER BY updated_at DESC")
        .await?
        .take(0)?;

    let body = html! {
        div class="mx-auto max-w-3xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-5xl" { "Journal" }
                div class="flex items-center gap-4" {
                    a href="/dashboard" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "← Dashboard" }
                    a href="/dashboard/posts/new" class="rounded-full bg-[color:var(--hot)] px-4 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "New post" }
                }
            }
            ul class="mt-8 divide-y rounded-lg border border-ink/10 bg-white" {
                @if posts.is_empty() {
                    li class="px-4 py-6 text-sm opacity-60" { "No posts yet. Write your first." }
                }
                @for p in &posts {
                    li class="flex items-center justify-between px-4 py-3" {
                        div {
                            a href=(format!("/dashboard/posts/{}/edit", p.slug)) class="font-medium hover:text-[color:var(--hot)]" { (p.title) }
                            div class="text-xs opacity-60" { (p.tag) " · " (p.published_date()) }
                        }
                        span class={ @if p.is_published() { "rounded-full bg-[color:color-mix(in_oklab,var(--hot)_16%,transparent)] px-2 py-0.5 text-xs uppercase tracking-widest text-[color:var(--hot)]" } @else { "rounded-full bg-black/5 px-2 py-0.5 text-xs uppercase tracking-widest opacity-60" } } {
                            (p.status)
                        }
                    }
                }
            }
        }
    };
    Ok(Html(shell("Journal — RBE Editor", "Manage RBE journal posts.", Nav::None, body).into_string()))
}

pub async fn post_new(_user: StaffUser) -> Html<String> {
    Html(editor(None, None).into_string())
}

pub async fn post_edit(_user: StaffUser, State(state): State<AppState>, Path(slug): Path<String>) -> AppResult<Html<String>> {
    let post: Option<Post> = state
        .db()
        .query("SELECT * FROM type::thing('post', $slug)")
        .bind(("slug", slug))
        .await?
        .take(0)?;
    let post = post.ok_or(AppError::NotFound)?;
    Ok(Html(editor(Some(&post), None).into_string()))
}

#[derive(Deserialize)]
pub struct PostForm {
    slug: String,
    title: String,
    #[serde(default)]
    excerpt: String,
    #[serde(default)]
    cover_url: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    body_md: String,
    #[serde(default)]
    status: String,
}

pub async fn post_save(
    user: StaffUser,
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let (f, uploaded_cover) = match parse_post_form(multipart).await {
        Ok(v) => v,
        Err(msg) => {
            let fallback = PostForm {
                slug: String::new(),
                title: String::new(),
                excerpt: String::new(),
                cover_url: String::new(),
                tag: String::new(),
                body_md: String::new(),
                status: "draft".to_string(),
            };
            return editor_error(&msg, &fallback).into_response();
        }
    };
    let title = f.title.trim();
    let mut slug = f.slug.trim().to_string();
    if slug.is_empty() {
        slug = slugify(title);
    } else {
        slug = slugify(&slug);
    }
    if title.is_empty() || slug.is_empty() {
        return editor_error("A title (and slug) is required.", &f).into_response();
    }
    let status = if f.status == "published" { "published" } else { "draft" };
    let cover = {
        if let Some(path) = uploaded_cover.as_deref() {
            Some(path)
        } else {
            let c = f.cover_url.trim();
            if c.is_empty() { None } else { Some(c) }
        }
    };
    let tag = if f.tag.trim().is_empty() { "Dispatch" } else { f.tag.trim() };

    match db::upsert_post(
        state.db(), &slug, title, f.excerpt.trim(), cover, tag,
        &f.body_md, status, user.email.as_str(), &now_rfc3339(),
    )
    .await
    {
        Ok(_) => Redirect::to("/dashboard/posts").into_response(),
        Err(_) => editor_error("Could not save the post. The slug may clash with another post.", &f).into_response(),
    }
}

fn editor_error(msg: &str, f: &PostForm) -> Html<String> {
    // Rebuild a Post-like view from the submitted values to re-fill the form.
    let post = Post {
        slug: f.slug.clone(),
        title: f.title.clone(),
        excerpt: f.excerpt.clone(),
        cover_url: (!f.cover_url.trim().is_empty()).then(|| f.cover_url.clone()),
        tag: f.tag.clone(),
        body_md: f.body_md.clone(),
        status: f.status.clone(),
        author: None,
        published_at: None,
        updated_at: None,
    };
    Html(editor(Some(&post), Some(msg)).into_string())
}

fn editor(post: Option<&Post>, error: Option<&str>) -> Markup {
    let val = |f: fn(&Post) -> String| post.map(f).unwrap_or_default();
    let slug = val(|p| p.slug.clone());
    let title = val(|p| p.title.clone());
    let excerpt = val(|p| p.excerpt.clone());
    let cover = post.and_then(|p| p.cover_url.clone()).unwrap_or_default();
    let tag = val(|p| p.tag.clone());
    let body = val(|p| p.body_md.clone());
    let published = post.map(|p| p.is_published()).unwrap_or(false);
    let heading = if post.is_some() { "Edit post" } else { "New post" };

    let field = "mt-1 w-full rounded-md border border-ink/20 bg-white px-3 py-2 text-sm outline-none focus:border-[color:var(--hot)]";
    let content = html! {
        div class="mx-auto max-w-2xl px-4 py-16" {
            div class="flex items-center justify-between" {
                h1 class="font-display text-4xl" { (heading) }
                a href="/dashboard/posts" class="text-sm uppercase tracking-widest opacity-60 hover:opacity-100" { "← All posts" }
            }
            @if let Some(err) = error { p class="mt-4 text-sm text-red-500" { (err) } }
            form method="post" action="/dashboard/posts" enctype="multipart/form-data" class="mt-6 space-y-4" {
                div {
                    label class="text-sm font-medium" { "Title" }
                    input name="title" value=(title) required class=(field);
                }
                div class="grid grid-cols-2 gap-4" {
                    div {
                        label class="text-sm font-medium" { "Slug" }
                        input name="slug" value=(slug) placeholder="auto from title" class=(field);
                    }
                    div {
                        label class="text-sm font-medium" { "Tag" }
                        input name="tag" value=(tag) placeholder="Essay" class=(field);
                    }
                }
                div {
                    label class="text-sm font-medium" { "Excerpt" }
                    input name="excerpt" value=(excerpt) placeholder="One-line summary" class=(field);
                }
                div {
                    label class="text-sm font-medium" { "Cover image upload" }
                    input type="file" name="cover_upload" accept="image/png,image/jpeg,image/webp,image/gif" class=(field);
                    p class="mt-2 text-xs opacity-60" { "Upload a JPG, PNG, WebP, or GIF. If you skip this, the URL field below is used." }
                }
                div {
                    label class="text-sm font-medium" { "Cover image URL (optional fallback)" }
                    input name="cover_url" value=(cover) class=(field);
                }
                div {
                    label class="text-sm font-medium" { "Body (Markdown)" }
                    textarea name="body_md" rows="16" class=(format!("{field} font-mono")) { (body) }
                }
                div class="flex items-center justify-between" {
                    label class="flex items-center gap-2 text-sm" {
                        select name="status" class="rounded-md border border-ink/20 bg-white px-3 py-2 text-sm" {
                            option value="draft" selected[!published] { "Draft" }
                            option value="published" selected[published] { "Published" }
                        }
                    }
                    button type="submit" class="rounded-full bg-[color:var(--hot)] px-6 py-2 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)]" { "Save" }
                }
            }
        }
    };
    shell("Editor — RBE Journal", "Write an RBE journal post.", Nav::None, content)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn md_to_html(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

async fn parse_post_form(mut multipart: Multipart) -> Result<(PostForm, Option<String>), String> {
    let mut form = PostForm {
        slug: String::new(),
        title: String::new(),
        excerpt: String::new(),
        cover_url: String::new(),
        tag: String::new(),
        body_md: String::new(),
        status: String::new(),
    };
    let mut uploaded_cover = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| "Could not read the journal form upload.".to_string())?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "cover_upload" => {
                let original = field.file_name().unwrap_or("cover").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| "Could not read the uploaded cover image.".to_string())?;
                if !bytes.is_empty() {
                    uploaded_cover = Some(save_cover_upload(&original, bytes.as_ref()).await?);
                }
            }
            "slug" => form.slug = field.text().await.unwrap_or_default(),
            "title" => form.title = field.text().await.unwrap_or_default(),
            "excerpt" => form.excerpt = field.text().await.unwrap_or_default(),
            "cover_url" => form.cover_url = field.text().await.unwrap_or_default(),
            "tag" => form.tag = field.text().await.unwrap_or_default(),
            "body_md" => form.body_md = field.text().await.unwrap_or_default(),
            "status" => form.status = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }

    Ok((form, uploaded_cover))
}

async fn save_cover_upload(filename: &str, bytes: &[u8]) -> Result<String, String> {
    let ext = detect_image_extension(filename, bytes)?;
    let dir = StdPath::new("static").join("uploads").join("journal");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|_| "Could not prepare the journal uploads folder.".to_string())?;
    let basename = format!("journal-{}.{}", uuid::Uuid::new_v4(), ext);
    let path = dir.join(&basename);
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|_| "Could not save the uploaded cover image.".to_string())?;
    Ok(format!("/static/uploads/journal/{basename}"))
}

fn detect_image_extension(filename: &str, bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Ok("png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("gif");
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok("webp");
    }

    let ext = StdPath::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpg"),
        "gif" => Ok("gif"),
        "webp" => Ok("webp"),
        _ => Err("Upload a JPG, PNG, WebP, or GIF image.".to_string()),
    }
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
