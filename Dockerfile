# PalStudio — Rust backend image.
#
# NOTE: the Rust stages require uesave to be a GIT dependency (the build
# context is this repo only; the sibling ../uesave-rs path checkout is not
# present). The path->git flip is the final Phase 7 step; this Dockerfile is
# buildable once that lands.
#
# 3 stages: bun UI build -> cargo-chef cached Rust build -> debian-slim runtime.

# ---- Stage 1: UI build (SvelteKit SPA, output -> /app/ui_build) ----
FROM oven/bun AS ui_builder

ARG PUBLIC_WS_URL=127.0.0.1:5174/ws

# Copy only what the UI build reads, so Rust-only changes do not invalidate the
# bun-install/vite layers: generate-sitemap.mjs reads ../../data/json.
COPY ps-ui /app/ps-ui
COPY data/json /app/data/json
WORKDIR /app/ps-ui
RUN echo "PUBLIC_WS_URL=${PUBLIC_WS_URL}" >.env; \
    echo "PUBLIC_DESKTOP_MODE=false" >>.env; \
    bun install --frozen-lockfile; \
    bun run build

# ---- Stage 2: Rust build (only ps-server) with cargo-chef layer caching ----
# Bump the pinned toolchain freely; edition 2021 workspace.
FROM lukemathwalker/cargo-chef:latest-rust-1.93-bookworm AS chef
WORKDIR /build

FROM chef AS planner
# The workspace lives at the repo root. Every member's manifest must be
# present for `cargo metadata` (hence ps-app/ps-web too), but only
# ps-server is ever compiled — the cook/build below scope to it, so
# tauri/webkit and the wasm deps are not pulled into this image.
COPY Cargo.toml Cargo.lock ./
COPY ps-core ps-core
COPY ps-db ps-db
COPY ps-app ps-app
COPY ps-lua-sys ps-lua-sys
COPY ps-plugin ps-plugin
COPY ps-server ps-server
COPY ps-desktop ps-desktop
COPY ps-web ps-web
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS rust_builder
COPY --from=planner /build/recipe.json recipe.json
# Dependency-only build for ps-server; cached until Cargo.toml/Cargo.lock change.
RUN cargo chef cook --release --locked --package ps-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY ps-core ps-core
COPY ps-db ps-db
COPY ps-app ps-app
COPY ps-lua-sys ps-lua-sys
COPY ps-plugin ps-plugin
COPY ps-server ps-server
COPY ps-desktop ps-desktop
COPY ps-web ps-web
RUN cargo build --release --locked --package ps-server

# ---- Stage 3: runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust_builder /build/target/release/ps-server /usr/local/bin/ps-server
COPY --from=ui_builder /app/ui_build /app/ui
COPY data /app/data

# WORKDIR doubles as the DB directory: the legacy psp.db import resolves to
# db_path.parent()/psp.db, i.e. /app/db/psp.db — same mounted volume as the
# new ps-rs.db. Drop a legacy psp.db into the volume to have it imported.
WORKDIR /app/db

EXPOSE 5174

CMD ["ps-server", "--host", "0.0.0.0", "--port", "5174", \
     "--ui-dir", "/app/ui", "--data-dir", "/app/data", \
     "--db", "/app/db/ps-rs.db"]
