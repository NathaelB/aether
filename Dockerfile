FROM rust:1.91-bookworm AS rust-build

WORKDIR /usr/local/src/aether

RUN cargo install sqlx-cli --no-default-features --features postgres

COPY Cargo.toml Cargo.lock ./
COPY libs/aether-auth/Cargo.toml ./libs/aether-auth/
COPY libs/aether-core/Cargo.toml ./libs/aether-core/
COPY libs/aether-api/Cargo.toml ./libs/aether-api/
COPY libs/aether-permission/Cargo.toml ./libs/aether-permission/
COPY libs/aether-crds/Cargo.toml ./libs/aether-crds/
COPY libs/aether-operator-core/Cargo.toml ./libs/aether-operator-core/


COPY apps/control-plane/Cargo.toml ./apps/control-plane/
COPY apps/operator/Cargo.toml ./apps/operator/

RUN \
    mkdir -p libs/aether-auth/src libs/aether-core/src libs/aether-api/src libs/aether-permission/src libs/aether-crds/src libs/aether-operator-core/src apps/control-plane/src apps/operator/src && \
    echo "fn main() {}" > libs/aether-auth/src/lib.rs && \
    echo "fn main() {}" > libs/aether-core/src/lib.rs && \
    echo "fn main() {}" > libs/aether-api/src/lib.rs && \
    echo "fn main() {}" > libs/aether-permission/src/lib.rs && \
    echo "fn main() {}" > libs/aether-crds/src/lib.rs && \
    echo "fn main() {}" > libs/aether-operator-core/src/lib.rs && \
    echo "fn main() {}" > apps/control-plane/src/main.rs && \
    echo "fn main() {}" > apps/operator/src/main.rs && \
    cargo build --release

COPY libs/aether-auth libs/aether-auth
COPY libs/aether-core libs/aether-core
COPY libs/aether-api libs/aether-api
COPY libs/aether-permission libs/aether-permission
COPY libs/aether-crds libs/aether-crds
COPY libs/aether-operator-core libs/aether-operator-core

COPY apps/control-plane apps/control-plane
COPY apps/operator apps/operator

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

COPY --from=rust-build /usr/local/src/aether/target/release/aether-control-plane /usr/local/bin/
COPY --from=rust-build /usr/local/src/aether/libs/aether-core/migrations /usr/local/src/aether/migrations
COPY --from=rust-build /usr/local/cargo/bin/sqlx /usr/local/bin/

EXPOSE 80

ENTRYPOINT [ "aether-control-plane" ]

FROM runtime AS operator

COPY --from=rust-build /usr/local/src/aether/target/release/aether-operator /usr/local/bin/

EXPOSE 80

ENTRYPOINT [ "aether-operator" ]

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
