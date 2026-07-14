# Palworld Save Pal — Rust backend image.
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

COPY . /app
WORKDIR /app/ui
RUN echo "PUBLIC_WS_URL=${PUBLIC_WS_URL}" >.env; \
    echo "PUBLIC_DESKTOP_MODE=false" >>.env; \
    bun install; \
    bun run build

# ---- Stage 2: Rust build (only psp-server) with cargo-chef layer caching ----
# Bump the pinned toolchain freely; edition 2021 workspace.
FROM rust:1.93-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
# The workspace lives at the repo root. psp-desktop is a member (needed to
# resolve the workspace) but is never compiled here — the cook/build below
# scope to psp-server, so tauri/webkit are not pulled into this image.
COPY Cargo.toml Cargo.lock ./
COPY psp-core psp-core
COPY psp-db psp-db
COPY psp-server psp-server
COPY psp-desktop psp-desktop
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS rust_builder
COPY --from=planner /build/recipe.json recipe.json
# Dependency-only build for psp-server; cached until Cargo.toml/Cargo.lock change.
RUN cargo chef cook --release --locked --package psp-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY psp-core psp-core
COPY psp-db psp-db
COPY psp-server psp-server
COPY psp-desktop psp-desktop
RUN cargo build --release --locked --package psp-server

# ---- Stage 3: runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust_builder /build/target/release/psp-server /usr/local/bin/psp-server
COPY --from=ui_builder /app/ui_build /app/ui
COPY data /app/data

# WORKDIR doubles as the DB directory: the legacy psp.db import resolves to
# db_path.parent()/psp.db, i.e. /app/db/psp.db — same mounted volume as the
# new psp-rs.db. Drop a legacy psp.db into the volume to have it imported.
WORKDIR /app/db

EXPOSE 5174

CMD ["psp-server", "--host", "0.0.0.0", "--port", "5174", \
     "--ui-dir", "/app/ui", "--data-dir", "/app/data", \
     "--db", "/app/db/psp-rs.db"]
