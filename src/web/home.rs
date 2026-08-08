//! Home page — ported from the reference `src/routes/index.tsx`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::AppResult;
use crate::models::Product;
use crate::state::AppState;

use super::layout::{Nav, shell};
use super::tee_mockup;

const MARQUEE: &[&str] = &[
    "RICH B ENERGY",
    "★",
    "HOT GIRLS GO TO THERAPY",
    "★",
    "CALL ME WHEN YOU'RE RICH",
    "★",
    "BAD B CLUB",
    "★",
    "ALL SUGAR NO DADDY",
    "★",
];

pub async fn home(State(state): State<AppState>) -> AppResult<Html<String>> {
    let featured: Vec<Product> = state
        .db()
        .query("SELECT * FROM product ORDER BY slug LIMIT 4")
        .await?
        .take(0)?;

    let body = html! {
        (hero())
        (marquee())
        (manifesto_strip())
        (featured_section(&featured))
        (for_her())
        (newsletter())
    };

    Ok(Html(
        shell(
            "RBE — Rich B Energy | Slogan tees for the unapologetic",
            "Rich B Energy. Loud, unapologetic slogan tees for women who don't shrink. Printed on demand, shipped worldwide.",
            Nav::None,
            body,
        )
        .into_string(),
    ))
}

#[derive(Deserialize)]
pub struct NewsletterSubscribeInput {
    email: String,
}

#[derive(Serialize)]
pub struct NewsletterSubscribeOutput {
    ok: bool,
    saved: bool,
    message: String,
}

pub async fn newsletter_subscribe(
    State(state): State<AppState>,
    Json(input): Json<NewsletterSubscribeInput>,
) -> impl IntoResponse {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(NewsletterSubscribeOutput {
                ok: false,
                saved: false,
                message: "Enter a valid email address.".into(),
            }),
        );
    }

    match db::create_newsletter_subscriber(state.db(), &email, "homepage").await {
        Ok(true) => (
            StatusCode::OK,
            Json(NewsletterSubscribeOutput {
                ok: true,
                saved: true,
                message: "You're in. Watch your inbox.".into(),
            }),
        ),
        Ok(false) => (
            StatusCode::OK,
            Json(NewsletterSubscribeOutput {
                ok: true,
                saved: false,
                message: "You're already on the list.".into(),
            }),
        ),
        Err(err) => {
            tracing::error!(?err, "failed to save newsletter subscriber");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(NewsletterSubscribeOutput {
                    ok: false,
                    saved: false,
                    message: "We couldn't save that right now. Please try again.".into(),
                }),
            )
        }
    }
}

fn hero() -> Markup {
    html! {
        section class="relative overflow-hidden bg-ink text-cream" {
            div class="pointer-events-none absolute inset-0 opacity-70"
                style="background:radial-gradient(60% 50% at 70% 30%, color-mix(in oklab, var(--hot) 40%, transparent), transparent 70%)" {}
            div class="relative mx-auto grid max-w-7xl gap-8 px-4 pb-20 pt-12 md:grid-cols-12 md:gap-6 md:px-8 md:pt-20" {
                div class="md:col-span-6" {
                    div class="mb-4 inline-block rounded-full border border-white/20 px-3 py-1 text-[10px] uppercase tracking-widest" {
                        "Vol. 01 — The RBE Manifesto Drop"
                    }
                    h1 class="font-display text-[16vw] leading-[0.85] tracking-tight md:text-[8.5rem]" {
                        "RICH B" br; span class="text-[color:var(--hot)]" { "ENERGY." }
                    }
                    p class="mt-6 max-w-lg font-serif-display text-2xl leading-snug text-cream/80" {
                        "Slogan tees for the ones who " em { "never" } " asked for permission. Loud, soft, grown, all at once. Made for every kind of woman, cut for the ones we don't see enough."
                    }
                    div class="mt-8 flex flex-wrap gap-3" {
                        a href="/shop"
                            class="group inline-flex items-center gap-2 rounded-full bg-[color:var(--hot)] px-6 py-3 text-sm font-semibold uppercase tracking-widest text-white transition hover:bg-white hover:text-ink" {
                            "Shop the drop →"
                        }
                        a href="/manifesto"
                            class="inline-flex items-center gap-2 rounded-full border border-white/30 px-6 py-3 text-sm font-semibold uppercase tracking-widest hover:border-[color:var(--hot)] hover:text-[color:var(--hot)]" {
                            "Read the manifesto"
                        }
                    }
                    div class="mt-10 flex items-center gap-6 text-[10px] uppercase tracking-widest opacity-60" {
                        span { "Est. 2026 · Australia" } span { "·" }
                        span { "Printed on demand" } span { "·" }
                        span { "Ships worldwide" }
                    }
                }
                div class="relative md:col-span-6" {
                    img src="/static/img/hero-tee.jpg"
                        alt="RBE Rich B Energy hot pink oversized t-shirt"
                        class="mx-auto w-full max-w-md md:max-w-none md:-mt-8"
                        style="filter:drop-shadow(0 30px 60px rgba(255, 43, 143, 0.35))";
                }
            }
        }
    }
}

fn marquee() -> Markup {
    html! {
        div class="border-y border-ink/10 bg-[color:var(--hot)] py-4 text-white" {
            div class="marquee" {
                div class="marquee-track font-display text-3xl tracking-wide" {
                    @for w in MARQUEE.iter().chain(MARQUEE.iter()) { span { (w) } }
                }
            }
        }
    }
}

fn manifesto_strip() -> Markup {
    html! {
        section class="bg-[color:var(--cream)] py-20" {
            div class="mx-auto grid max-w-6xl gap-10 px-4 md:grid-cols-2 md:px-8" {
                div {
                    div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "The RBE thesis" }
                    h2 class="mt-3 font-display text-5xl leading-none md:text-6xl" {
                        "SOFT LIFE." br; "HARD LINES."
                    }
                }
                div class="space-y-4 font-serif-display text-xl leading-snug text-ink/80" {
                    p { "RBE is a love letter to the women who grew up being told to be smaller, quieter, more agreeable, and chose the opposite." }
                    p { "Every tee is a one-line thesis. Wear the one that already lives in your chest." }
                    a href="/manifesto" class="inline-block pt-2 font-body text-sm uppercase tracking-widest text-[color:var(--hot)] underline underline-offset-4" {
                        "Read the full manifesto →"
                    }
                }
            }
        }
    }
}

fn featured_section(featured: &[Product]) -> Markup {
    html! {
        section class="bg-[color:var(--blush)]/40 py-20" {
            div class="mx-auto max-w-7xl px-4 md:px-8" {
                div class="mb-10 flex items-end justify-between" {
                    div {
                        div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Featured" }
                        h2 class="mt-2 font-display text-5xl md:text-6xl" { "THE DROP" }
                    }
                    a href="/shop" class="text-sm uppercase tracking-widest underline underline-offset-4" { "See all" }
                }
                div class="grid grid-cols-2 gap-4 md:grid-cols-4 md:gap-6" {
                    @for p in featured {
                        a href=(format!("/shop/{}", p.slug)) class="group block" {
                            div class="overflow-hidden rounded-md bg-white p-3 transition group-hover:-translate-y-1 group-hover:shadow-xl group-hover:shadow-[color:var(--hot)]/20" {
                                (tee_mockup(&p.image, &p.slogan_flat(), &p.tee_color))
                            }
                            div class="mt-3 flex items-center justify-between" {
                                div class="text-sm font-semibold uppercase" { (p.slogan_first_line()) }
                                div class="text-sm" { "$" (p.price) }
                            }
                            div class="text-xs uppercase tracking-widest opacity-60" { (p.vibe) }
                        }
                    }
                }
            }
        }
    }
}

fn for_her() -> Markup {
    html! {
        section class="bg-ink py-20 text-cream" {
            div class="mx-auto grid max-w-6xl gap-10 px-4 md:grid-cols-2 md:px-8" {
                div class="font-display text-6xl leading-none md:text-8xl" {
                    "FOR EVERY" br; span class="text-[color:var(--hot)]" { "B" } "." br;
                    span class="text-cream/40" { "FOR OURS FIRST." }
                }
                div class="space-y-4 text-sm leading-relaxed opacity-80" {
                    p { "RBE is built for every kind of woman, but we designed it thinking about Black women first. The way our slang gets borrowed, our style gets copied, our energy gets sold back to us. We're keeping this one." }
                    p { "Every drop, we spotlight a Black woman-owned brand or fund. No hashtag; a line on the receipt." }
                    a href="/manifesto" class="inline-block pt-2 text-xs uppercase tracking-widest text-[color:var(--hot)] underline underline-offset-4" { "Our promise →" }
                }
            }
        }
    }
}

fn newsletter() -> Markup {
    html! {
        section class="bg-[color:var(--cream)] py-20" {
            div class="mx-auto max-w-3xl px-4 text-center md:px-8" {
                h3 class="font-display text-5xl md:text-6xl" {
                    "JOIN THE " span class="text-[color:var(--hot)]" { "CLUB" }
                }
                p class="mt-3 font-serif-display text-xl opacity-70" {
                    "Early access to drops, 10% off your first tee, and zero spam."
                }
                form class="mx-auto mt-6 flex max-w-md gap-2"
                    x-data="{ email: '', status: 'idle', message: '' }"
                    "@submit.prevent"="
                        status = 'loading';
                        message = '';
                        fetch('/api/newsletter/subscribe', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ email })
                        })
                        .then(async (response) => {
                            const data = await response.json();
                            status = data.ok ? 'success' : 'error';
                            message = data.message || 'Please try again.';
                            if (data.ok) email = '';
                        })
                        .catch(() => {
                            status = 'error';
                            message = 'Please try again.';
                        });
                    " {
                    input type="email" required placeholder="your@email.com" "x-model"="email"
                        class="flex-1 rounded-full border border-ink/20 bg-white px-5 py-3 text-sm outline-none focus:border-[color:var(--hot)]";
                    button type="submit"
                        ":disabled"="status === 'loading'"
                        class="rounded-full bg-[color:var(--hot)] px-6 py-3 text-sm font-semibold uppercase tracking-widest text-white hover:bg-[color:var(--crimson)] disabled:cursor-wait disabled:opacity-70" {
                        span x-show="status !== 'loading'" { "Join" }
                        span x-show="status === 'loading'" { "Saving..." }
                    }
                }
                p class="mt-3 text-sm text-ink/70" x-text="message" x-show="message" {}
            }
        }
    }
}
