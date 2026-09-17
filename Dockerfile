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
    && apt-get install -y --no-install-recommends ca-certificates libstdc++6 curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system palstudio \
    && useradd --system --gid palstudio --home-dir /nonexistent --shell /usr/sbin/nologin palstudio \
    && install -d --owner palstudio --group palstudio --mode 0750 /app/db

COPY --from=rust_builder /build/target/release/ps-server /usr/local/bin/ps-server
COPY --from=ui_builder /app/ui_build /app/ui
COPY data /app/data

RUN chown root:root /usr/local/bin/ps-server \
    && chmod 0755 /usr/local/bin/ps-server \
    && chown -R root:root /app/ui /app/data \
    && find /app/ui /app/data -type d -exec chmod 0755 {} + \
    && find /app/ui /app/data -type f -exec chmod 0644 {} +

WORKDIR /app/db

EXPOSE 5174

# The SPA middleware serves ui/index.html at "/" (200) once the server is up,
# which makes it a suitable liveness probe without a dedicated health route.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS -o /dev/null http://127.0.0.1:5174/ || exit 1

# The container listener uses all container interfaces so Docker can publish
# it; the compose file binds the host side to loopback by default, while the
# application policy remains the authority for admitted peers.
USER palstudio
CMD ["ps-server", "--hosted", "--host", "0.0.0.0", \
     "--ui-dir", "/app/ui", "--data-dir", "/app/data", \
     "--db", "/app/db/ps-rs.db"]
