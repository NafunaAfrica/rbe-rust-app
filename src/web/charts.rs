//! Tiny server-rendered SVG charts — no JavaScript charting library. Colors use
//! `currentColor` / CSS vars so they stay theme-aware.

use maud::{Markup, html};

fn f(x: f64) -> String {
    format!("{x:.1}")
}

/// A vertical bar chart. `data` is (label, value); bars use `accent` (a CSS
/// color, e.g. `var(--hot)`). Renders a responsive SVG.
pub fn bar_chart(data: &[(String, f64)], accent: &str) -> Markup {
    const W: f64 = 720.0;
    const H: f64 = 200.0;
    const PAD_B: f64 = 26.0;
    const PAD_T: f64 = 14.0;
    const GAP: f64 = 6.0;

    let n = data.len().max(1) as f64;
    let max = data.iter().map(|(_, v)| *v).fold(1.0_f64, f64::max);
    let bw = ((W - GAP * (n - 1.0)) / n).max(1.0);
    let plot_h = H - PAD_B - PAD_T;

    struct Bar {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        label: String,
    }
    let bars: Vec<Bar> = data
        .iter()
        .enumerate()
        .map(|(i, (label, v))| {
            let bh = ((v / max) * plot_h).max(0.0);
            Bar {
                x: i as f64 * (bw + GAP),
                y: PAD_T + (plot_h - bh),
                w: bw,
                h: bh,
                label: label.clone(),
            }
        })
        .collect();
    let label_every = (bars.len() / 7).max(1);

    html! {
        svg viewBox=(format!("0 0 {W} {H}")) style="width:100%;height:auto;display:block" role="img" aria-label="Chart" {
            // baseline
            line x1="0" y1=(f(PAD_T + plot_h)) x2=(f(W)) y2=(f(PAD_T + plot_h)) stroke="currentColor" stroke-width="1" style="opacity:.12" {}
            @for b in &bars {
                rect x=(f(b.x)) y=(f(b.y)) width=(f(b.w)) height=(f(b.h)) rx="2" fill=(accent) {}
            }
            @for (i, b) in bars.iter().enumerate() {
                @if i % label_every == 0 {
                    text x=(f(b.x + b.w / 2.0)) y=(f(H - 8.0)) text-anchor="middle" font-size="9" fill="currentColor" style="opacity:.5" {
                        (b.label)
                    }
                }
            }
        }
    }
}
