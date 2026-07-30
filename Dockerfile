# syntax=docker/dockerfile:1

# ─────────────────────────────────────────────────────────────────────────────
# RBE — single-container build. No Node anywhere:
#   1. `css` stage compiles Tailwind with the standalone CLI.
#   2. `build` stage compiles the Rust binary (cargo-chef caches deps).
#   3. runtime is a slim Debian image with just the binary + static assets.
# ─────────────────────────────────────────────────────────────────────────────

# --- 1. Tailwind CSS (standalone, no Node) ---
FROM debian:bookworm-slim AS css
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /usr/local/bin/tailwindcss \
      https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 \
    && chmod +x /usr/local/bin/tailwindcss
COPY styles ./styles
COPY src ./src
RUN tailwindcss -i styles/input.css -o /out/app.css --minify

# --- 2. Rust build with cargo-chef dependency caching ---
FROM rust:1-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
# surrealdb-core is huge; drop debuginfo to keep the compiler's memory in check.
ENV CARGO_PROFILE_RELEASE_DEBUG=0
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin rbe

# --- 3. Runtime ---
FROM debian:bookworm-slim AS runtime
# curl is required so Coolify's in-container health check (GET /health) works —
# a slim image has neither curl nor wget, which silently fails the deploy.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 rbe
WORKDIR /app

COPY --from=build /app/target/release/rbe /usr/local/bin/rbe
COPY static ./static
COPY --from=css /out/app.css ./static/app.css

# SurrealDB data lives here; mount a volume to persist it.
RUN mkdir -p /data && chown -R rbe:rbe /data /app
ENV RBE_DATA_DIR=/data/surreal
ENV RBE_BIND_ADDR=0.0.0.0:8080
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1
USER rbe
CMD ["rbe"]
