//! Static content pages — manifesto and journal (ported from the reference
//! `src/routes/manifesto.tsx` and `journal.tsx`).

use axum::response::Html;
use maud::html;

use super::layout::{Nav, shell};

const RULES: &[&str] = &[
    "We do not shrink.",
    "We do not apologize for our tone.",
    "We take up the room we paid for.",
    "We spend on ourselves first.",
    "We rest. Loudly.",
    "We compliment other women in public.",
    "We invoice on time.",
    "We call it what it is.",
    "We were the blueprint.",
];

pub async fn manifesto() -> Html<String> {
    let body = html! {
        div class="bg-[color:var(--cream)]" {
            section class="mx-auto max-w-4xl px-4 pt-20 md:px-8" {
                div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "The manifesto" }
                h1 class="mt-3 font-display text-6xl leading-[0.9] md:text-[10rem]" {
                    "RICH B" br; span class="text-[color:var(--hot)]" { "ENERGY" } br; "IS A LIFESTYLE."
                }
            }
            section class="mx-auto max-w-4xl px-4 py-16 md:px-8" {
                p class="font-serif-display text-3xl leading-snug text-ink/80 md:text-4xl" {
                    "RBE started at a kitchen table, in a group chat that ran too long. It's for the women who are the plan, the plug, and the punchline, the ones who built something out of nothing and then had to explain it twice."
                }
                p class="mt-6 font-serif-display text-3xl leading-snug text-ink/80 md:text-4xl" {
                    "We design for " em { "every" } " kind of woman, and we design for Black women first, because we've watched our slang, our style, and our softness get borrowed without credit for the last time."
                }
            }
            section class="bg-[color:var(--hot)] py-20 text-white" {
                div class="mx-auto max-w-4xl px-4 md:px-8" {
                    div class="text-xs uppercase tracking-widest opacity-80" { "The rules" }
                    ol class="mt-6 space-y-4" {
                        @for (i, rule) in RULES.iter().enumerate() {
                            li class="flex gap-6 border-b border-white/20 pb-4" {
                                span class="font-display text-3xl opacity-60 md:text-5xl" { (format!("{:02}", i + 1)) }
                                span class="font-display text-3xl leading-tight md:text-5xl" { (rule) }
                            }
                        }
                    }
                }
            }
            section class="mx-auto max-w-3xl px-4 py-20 text-center md:px-8" {
                h2 class="font-display text-5xl md:text-7xl" {
                    "NOW GO " span class="text-[color:var(--hot)]" { "SHOP" } "."
                }
                a href="/shop" class="mt-8 inline-block rounded-full bg-ink px-8 py-4 font-display text-lg uppercase tracking-widest text-cream hover:bg-[color:var(--hot)]" {
                    "The drop →"
                }
            }
        }
    };
    Html(shell("Manifesto — RBE Rich B Energy", "The RBE manifesto. Soft life. Hard lines. For every kind of woman, for ours first.", Nav::Manifesto, body).into_string())
}

const POSTS: &[(&str, &str, &str)] = &[
    ("On being called 'intimidating' one more time", "Essay", "This week"),
    ("The soft-life spreadsheet that saved my summer", "Money", "Last week"),
    ("A playlist for closing tabs and closing deals", "Sound", "Aug"),
    ("Five Black-woman-owned tailors we love", "Guide", "Jul"),
];

pub async fn journal() -> Html<String> {
    let body = html! {
        div class="mx-auto max-w-6xl px-4 py-20 md:px-8" {
            div class="text-xs uppercase tracking-widest text-[color:var(--hot)]" { "Dispatches" }
            h1 class="mt-2 font-display text-6xl md:text-8xl" { "JOURNAL" }
            div class="mt-12 grid gap-6 md:grid-cols-2" {
                @for (title, tag, date) in POSTS {
                    article class="rounded-lg border border-ink/10 bg-white p-8" {
                        div class="flex items-center justify-between text-xs uppercase tracking-widest opacity-60" {
                            span { (tag) } span { (date) }
                        }
                        h2 class="mt-4 font-serif-display text-3xl leading-snug" { (title) }
                        div class="mt-6 text-xs uppercase tracking-widest text-[color:var(--hot)]/70" { "Coming soon" }
                    }
                }
            }
        }
    };
    Html(shell("Journal — RBE", "Essays, playlists, and dispatches from the RBE club.", Nav::Journal, body).into_string())
}
