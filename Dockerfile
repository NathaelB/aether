# 1.94 at least: the AWS SDK the object store adapter uses declares that as its
# minimum, and 1.91 refuses the whole workspace before compiling a line. A
# developer toolchain is newer than this image, so the mismatch only shows up
# here.
FROM rust:1.96-bookworm AS chef

WORKDIR /usr/local/src/aether

RUN cargo install cargo-chef --version 0.1.77 --locked && \
    cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features postgres

# --- Plan: extract a recipe of the workspace dependency graph ----------
# Only the manifests and the lockfile shape this file, so a source-only
# change leaves the recipe -- and therefore the cook layer -- untouched.
FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Cook: build every dependency from the recipe ----------------------
# Cached as long as the dependency graph is stable. This is the layer
# that used to be faked with dummy sources, and the reason the per-crate
# COPY list existed at all.
FROM chef AS builder

ENV SQLX_OFFLINE=true

COPY --from=planner /usr/local/src/aether/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# --- Build the workspace binaries --------------------------------------
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates=20230311+deb12u1 \
    libssl3=3.0.17-1~deb12u2 && \
    rm -rf /var/lib/apt/lists/* && \
    addgroup \
    --system \
    --gid 1000 \
    aether && \
    adduser \
    --system \
    --no-create-home \
    --disabled-login \
    --uid 1000 \
    --gid 1000 \
    aether

USER aether

FROM runtime AS control-plane

COPY --from=builder /usr/local/src/aether/target/release/aether-control-plane /usr/local/bin/
COPY --from=builder --chown=aether:aether /usr/local/src/aether/libs/aether-core/migrations /usr/local/src/aether/migrations
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/

EXPOSE 80

ENTRYPOINT [ "aether-control-plane" ]

FROM runtime AS operator

COPY --from=builder /usr/local/src/aether/target/release/aether-operator /usr/local/bin/

EXPOSE 80

ENTRYPOINT [ "aether-operator" ]

FROM runtime AS herald

COPY --from=builder /usr/local/src/aether/target/release/herald /usr/local/bin/

ENTRYPOINT [ "herald" ]

FROM runtime AS genesis

COPY --from=builder /usr/local/src/aether/target/release/genesis /usr/local/bin/

ENTRYPOINT [ "genesis" ]

FROM node:24.12-alpine AS console-build

WORKDIR /usr/local/src/aether

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"

RUN \
    corepack enable && \
    corepack prepare pnpm@9.15.0 --activate && \
    apk --no-cache add dumb-init=1.2.5-r3

COPY apps/console/package.json apps/console/pnpm-lock.yaml ./

RUN pnpm install --frozen-lockfile

COPY apps/console/ .

RUN pnpm run build

FROM nginx:1.28.0-alpine3.21-slim AS console

COPY --from=console-build /usr/local/src/aether/dist /usr/local/src/aether
COPY apps/console/nginx.conf /etc/nginx/conf.d/default.conf
COPY apps/console/docker-entrypoint.sh /docker-entrypoint.d/docker-entrypoint.sh

RUN chmod +x /docker-entrypoint.d/docker-entrypoint.sh
