//! Base HTML shell: `<head>`, site header, footer, and the Alpine-powered cart
//! drawer. Every page composes its content into `shell(...)`.

use maud::{DOCTYPE, Markup, PreEscaped, html};

const SITE_URL: &str = "https://rbemood.com";
const DEFAULT_OG_IMAGE: &str = "https://rbemood.com/static/img/hero-tee.jpg";

/// Which nav item is active, for highlighting.
#[derive(PartialEq)]
pub enum Nav {
    None,
    Shop,
    Manifesto,
    Journal,
}

pub fn shell(title: &str, description: &str, nav: Nav, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                meta name="description" content=(description);
                meta property="og:site_name" content="RBE";
                meta property="og:type" content="website";
                meta property="og:title" content=(title);
                meta property="og:description" content=(description);
                meta property="og:url" content=(SITE_URL);
                meta property="og:image" content=(DEFAULT_OG_IMAGE);
                meta property="og:image:alt" content="RBE Rich B Energy oversized slogan tee";
                meta property="og:image:width" content="1200";
                meta property="og:image:height" content="630";
                meta name="twitter:card" content="summary_large_image";
                meta name="twitter:title" content=(title);
                meta name="twitter:description" content=(description);
                meta name="twitter:image" content=(DEFAULT_OG_IMAGE);
                link rel="canonical" href=(SITE_URL);
                link rel="icon" href="/static/img/fav.svg" type="image/svg+xml";
                link rel="alternate icon" href="/static/favicon.ico" type="image/x-icon";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous";
                link rel="stylesheet"
                    href="https://fonts.googleapis.com/css2?family=Anton&family=Instrument+Serif:ital@0;1&family=Inter:wght@400;500;600;700&display=swap";
                link rel="stylesheet" href="/static/app.css";
                script src="/static/vendor/alpine.min.js" defer {}
                script src="/static/vendor/htmx.min.js" defer {}
                (page_loader_script())
                (cart_store_script())
            }
            body class="page-shell" {
                div id="page-loader"
                    class="fixed inset-0 z-[120] flex items-center justify-center bg-[color:var(--cream)] transition duration-300 ease-out" {
                    div class="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(255,255,255,0.22),transparent_62%)]" {}
                    div class="relative flex flex-col items-center gap-5 text-center" {
                        div class="page-loader__mark font-display text-6xl leading-none tracking-tight text-ink md:text-8xl" {
                            "RBE" span class="text-[color:var(--hot)]" { "." }
                        }
                        div class="h-[2px] w-28 overflow-hidden rounded-full bg-ink/10" {
                            div class="page-loader__line h-full w-full bg-[color:var(--hot)]" {}
                        }
                    }
                }
                div x-data="{}" class="flex min-h-screen flex-col" {
                    (header(nav))
                    main class="flex-1" { (body) }
                    (footer())
                    (cart_drawer())
                }
            }
        }
    }
}

fn header(nav: Nav) -> Markup {
    let link = |href: &str, label: &str, active: bool| {
        let cls = if active {
            "text-[color:var(--hot)]"
        } else {
            "hover:text-[color:var(--hot)]"
        };
        html! { a href=(href) class=(cls) { (label) } }
    };
    html! {
        header x-data="{ navOpen: false }"
            class="sticky top-0 z-40 border-b border-ink/10 bg-[color:var(--cream)]/85 backdrop-blur" {
            div class="mx-auto flex max-w-7xl items-center justify-between px-4 py-3 md:px-8" {
                button class="md:hidden" "@click"="navOpen = !navOpen" aria-label="Menu" { "☰" }
                a href="/" class="font-display text-2xl tracking-tight text-ink" {
                    "RBE" span class="text-[color:var(--hot)]" { "." }
                }
                nav class="hidden items-center gap-8 text-sm uppercase tracking-widest md:flex" {
                    (link("/shop", "Shop", nav == Nav::Shop))
                    (link("/manifesto", "Manifesto", nav == Nav::Manifesto))
                    (link("/journal", "Journal", nav == Nav::Journal))
                    a href="/account" class="hover:text-[color:var(--hot)]" { "Account" }
                }
                button "@click"="$store.cart.open = true"
                    class="relative flex items-center gap-2 rounded-full border border-ink/20 px-3 py-1.5 text-xs uppercase tracking-widest hover:border-[color:var(--hot)] hover:text-[color:var(--hot)]" {
                    span { "Bag" }
                    span class="min-w-[1.25rem] rounded-full bg-[color:var(--hot)] px-1.5 py-0.5 text-center text-[10px] font-bold text-white"
                        x-text="$store.cart.count" { "0" }
                }
            }
            nav x-show="navOpen" x-cloak
                class="flex flex-col gap-2 border-t border-ink/10 px-4 py-3 text-sm uppercase tracking-widest md:hidden" {
                a href="/shop" { "Shop" }
                a href="/manifesto" { "Manifesto" }
                a href="/journal" { "Journal" }
                a href="/account" { "Account" }
            }
        }
    }
}

fn footer() -> Markup {
    html! {
        footer class="mt-24 border-t border-ink/10 bg-ink text-cream" {
            div class="mx-auto grid max-w-7xl gap-10 px-4 py-16 md:grid-cols-4 md:px-8" {
                div class="md:col-span-2" {
                    div class="font-display text-5xl leading-none" {
                        "RICH B" br; span class="text-[color:var(--hot)]" { "ENERGY." }
                    }
                    p class="mt-4 max-w-sm text-sm opacity-70" {
                        "Slogan tees for women who don't shrink. Designed by a Black woman with a dream in Australia, printed on demand so nothing goes to waste."
                    }
                }
                div {
                    div class="text-xs uppercase tracking-widest opacity-60" { "Shop" }
                    ul class="mt-3 space-y-2 text-sm" {
                        li { a href="/shop" { "All tees" } }
                        li { a href="/shop" { "New drops" } }
                        li { a href="/manifesto" { "Manifesto" } }
                    }
                }
                div {
                    div class="text-xs uppercase tracking-widest opacity-60" { "Care" }
                    ul class="mt-3 space-y-2 text-sm opacity-80" {
                        li { "Shipping worldwide" }
                        li { "30-day returns" }
                        li { "hello@rbe.club" }
                    }
                }
            }
            div class="border-t border-white/10 py-4 text-center text-xs uppercase tracking-widest opacity-60" {
                "© 2026 RBE Rich B Energy"
            }
        }
    }
}

/// The slide-in cart, driven entirely by the Alpine `cart` store.
fn cart_drawer() -> Markup {
    html! {
        div x-show="$store.cart.open" x-cloak
            "@click"="$store.cart.open = false"
            class="fixed inset-0 z-50 bg-ink/50 backdrop-blur-sm" {}
        aside x-show="$store.cart.open" x-cloak
            "x-transition:enter-start"="translate-x-full" "x-transition:enter-end"="translate-x-0"
            class="fixed right-0 top-0 z-50 flex h-full w-full max-w-md flex-col bg-[color:var(--cream)] shadow-2xl" {
            div class="flex items-center justify-between border-b border-ink/10 px-5 py-4" {
                div class="font-display text-2xl" { "YOUR BAG" }
                button "@click"="$store.cart.open = false" aria-label="Close" { "✕" }
            }
            div class="flex-1 overflow-auto px-5 py-4" {
                template x-if="$store.cart.items.length === 0" {
                    div class="py-16 text-center text-sm opacity-60" { "Empty. Go find your slogan." }
                }
                ul class="space-y-4" {
                    template x-for="it in $store.cart.items" ":key"="it.id" {
                        li class="flex gap-4 border-b border-ink/10 pb-4" {
                            div class="h-24 w-24 shrink-0 overflow-hidden rounded bg-white" {
                                img ":src"="it.image" ":alt"="it.title" class="h-full w-full object-cover";
                            }
                            div class="flex flex-1 flex-col" {
                                div class="flex justify-between" {
                                    div {
                                        div class="text-sm font-semibold uppercase" x-text="it.title" {}
                                        template x-if="it.size" {
                                            div class="text-xs opacity-60" { "Size " span x-text="it.size" {} }
                                        }
                                    }
                                    button class="text-xs uppercase text-[color:var(--hot)]"
                                        "@click"="$store.cart.remove(it.id)" { "Remove" }
                                }
                                div class="mt-auto flex items-center justify-between" {
                                    div class="inline-flex items-center gap-2 rounded border border-ink/20" {
                                        button class="px-2 py-1" "@click"="$store.cart.setQty(it.id, it.qty - 1)" { "−" }
                                        span class="min-w-[1.5rem] text-center text-sm" x-text="it.qty" {}
                                        button class="px-2 py-1" "@click"="$store.cart.setQty(it.id, it.qty + 1)" { "+" }
                                    }
                                    div class="font-semibold" x-text="'$' + (it.price * it.qty).toFixed(2)" {}
                                }
                            }
                        }
                    }
                }
            }
            div class="border-t border-ink/10 px-5 py-4" {
                div class="mb-3 flex justify-between text-sm" {
                    span class="uppercase tracking-widest opacity-60" { "Subtotal" }
                    span class="font-semibold" x-text="'$' + $store.cart.subtotal.toFixed(2)" {}
                }
                button "@click"="$store.cart.checkout()"
                    ":disabled"="$store.cart.items.length === 0"
                    class="inline-flex w-full items-center justify-center gap-2 rounded-full bg-[color:var(--hot)] py-3 font-display text-lg uppercase tracking-wider text-white transition hover:bg-[color:var(--crimson)] disabled:opacity-40" {
                    "Checkout"
                }
                p class="mt-2 text-center text-[10px] uppercase tracking-widest opacity-50" {
                    "Printed on demand · Ships in 3–5 days"
                }
            }
        }
    }
}

fn cart_store_script() -> Markup {
    PreEscaped(
        r#"<style>[x-cloak]{display:none!important}</style>
<script>
document.addEventListener('alpine:init', () => {
  Alpine.store('cart', {
    items: JSON.parse(localStorage.getItem('rbe_cart_v2') || '[]'),
    open: false,
    init(){ if (location.hash === '#cart') this.open = true; },
    save(){ localStorage.setItem('rbe_cart_v2', JSON.stringify(this.items)); },
    add(item){
      const i = this.items.findIndex(x => x.id === item.id);
      if (i >= 0) this.items[i].qty += item.qty; else this.items.push(item);
      this.save(); this.open = true;
    },
    remove(id){ this.items = this.items.filter(x => x.id !== id); this.save(); },
    setQty(id, q){
      const it = this.items.find(x => x.id === id);
      if (it) it.qty = Math.max(0, q);
      this.items = this.items.filter(x => x.qty > 0); this.save();
    },
    get count(){ return this.items.reduce((s,i) => s + i.qty, 0); },
    get subtotal(){ return this.items.reduce((s,i) => s + i.price * i.qty, 0); },
    async checkout(){
      if (this.items.length === 0) return;
      const res = await fetch('/api/checkout', {
        method:'POST', headers:{'Content-Type':'application/json'},
        body: JSON.stringify({ items: this.items.map(i => ({ slug: i.slug, size: i.size, qty: i.qty })) })
      });
      const data = await res.json().catch(() => ({}));
      if (res.status === 401 && data.login_required){
        // Bounce to sign-in, then return to the shop with the bag re-opened.
        const next = encodeURIComponent(location.pathname + '#cart');
        window.location = '/account/login?next=' + next;
        return;
      }
      if (!res.ok){ alert(data.error || 'Checkout failed. Please try again.'); return; }
      const u = new URL(data.url); u.searchParams.set('channel','online_store');
      window.location = u.toString();
    }
  });
});
</script>"#
        .to_string(),
    )
}

fn page_loader_script() -> Markup {
    PreEscaped(
        r#"<style>
#page-loader.is-hidden {
  opacity: 0;
  visibility: hidden;
}
#page-loader.is-active {
  opacity: 1;
  visibility: visible;
}
.page-loader__mark {
  animation: page-loader-breathe 1.2s ease-in-out infinite alternate;
}
.page-loader__line {
  transform-origin: left center;
  animation: page-loader-line 1.1s ease-in-out infinite;
}
@keyframes page-loader-breathe {
  from { transform: translateY(0); opacity: 0.82; }
  to { transform: translateY(-2px); opacity: 1; }
}
@keyframes page-loader-line {
  0% { transform: translateX(-105%) scaleX(0.4); }
  55% { transform: translateX(0%) scaleX(0.95); }
  100% { transform: translateX(105%) scaleX(0.5); }
}
</style>
<script>
(() => {
  const getLoader = () => document.getElementById('page-loader');
  let showTimer = null;

  const hideLoader = () => {
    const loader = getLoader();
    if (!loader) return;
    window.clearTimeout(showTimer);
    loader.classList.add('is-hidden');
    loader.classList.remove('is-active');
  };

  const activateLoader = () => {
    const loader = getLoader();
    if (!loader) return;
    loader.classList.remove('is-hidden');
    loader.classList.add('is-active');
  };

  const queueLoader = () => {
    const loader = getLoader();
    if (!loader) return;
    window.clearTimeout(showTimer);
    showTimer = window.setTimeout(() => {
      activateLoader();
    }, 140);
  };

  const isInternalNav = (link) => {
    const href = link.getAttribute('href');
    if (!href || href.startsWith('#') || href.startsWith('mailto:') || href.startsWith('tel:')) return false;
    if (link.target === '_blank' || link.hasAttribute('download')) return false;
    try {
      const url = new URL(link.href, window.location.origin);
      return url.origin === window.location.origin && url.href !== window.location.href;
    } catch {
      return false;
    }
  };

  document.addEventListener('click', (event) => {
    const link = event.target.closest('a');
    if (!link || event.defaultPrevented || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    if (isInternalNav(link)) queueLoader();
  });

  document.addEventListener('submit', () => queueLoader());
  window.addEventListener('pageshow', hideLoader);
  window.addEventListener('load', () => window.setTimeout(hideLoader, 120), { once: true });
})();
</script>"#
            .to_string(),
    )
}
