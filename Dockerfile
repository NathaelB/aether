FROM rust:1.91-bookworm AS rust-build

WORKDIR /usr/local/src/aether

RUN cargo install sqlx-cli --no-default-features --features postgres

ENV SQLX_OFFLINE=false

COPY Cargo.toml Cargo.lock ./
COPY libs/aether-auth/Cargo.toml ./libs/aether-auth/
COPY libs/aether-core/Cargo.toml ./libs/aether-core/
COPY libs/aether-api/Cargo.toml ./libs/aether-api/
COPY libs/aether-permission/Cargo.toml ./libs/aether-permission/
COPY libs/aether-crds/Cargo.toml ./libs/aether-crds/
COPY libs/aether-operator-core/Cargo.toml ./libs/aether-operator-core/
COPY libs/aether-herald-core/Cargo.toml ./libs/aether-herald-core/
COPY libs/aether-domain/Cargo.toml ./libs/aether-domain/
COPY libs/aether-postgres/Cargo.toml ./libs/aether-postgres/
COPY libs/aether-persistence/Cargo.toml ./libs/aether-persistence/

COPY apps/control-plane/Cargo.toml ./apps/control-plane/
COPY apps/operator/Cargo.toml ./apps/operator/

COPY docker/create-dummy-sources.sh .
RUN chmod +x create-dummy-sources.sh && \
    ./create-dummy-sources.sh && \
    cargo build --release

COPY libs/aether-auth libs/aether-auth
COPY libs/aether-core libs/aether-core
COPY libs/aether-api libs/aether-api
COPY libs/aether-permission libs/aether-permission
COPY libs/aether-crds libs/aether-crds
COPY libs/aether-operator-core libs/aether-operator-core
COPY libs/aether-herald-core libs/aether-herald-core
COPY libs/aether-domain ./libs/aether-domain
COPY libs/aether-postgres ./libs/aether-postgres
COPY libs/aether-persistence ./libs/aether-persistence

COPY .sqlx .sqlx
COPY apps/control-plane apps/control-plane
COPY apps/operator apps/operator



RUN \
    touch libs/aether-auth/src/lib.rs && \
    touch libs/aether-core/src/lib.rs && \
    touch libs/aether-api/src/lib.rs && \
    touch libs/aether-permission/src/lib.rs && \
    touch libs/aether-crds/src/lib.rs && \
    touch libs/aether-operator-core/src/lib.rs && \
    touch libs/aether-herald-core/src/lib.rs && \
    touch libs/aether-domain/src/lib.rs && \
    touch libs/aether-postgres/src/lib.rs && \
    touch libs/aether-persistence/src/lib.rs && \

    touch apps/control-plane/src/main.rs && \
    touch apps/operator/src/main.rs && \
    SQLX_OFFLINE=true cargo build --release

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
COPY --from=rust-build --chown=aether:aether /usr/local/src/aether/libs/aether-core/migrations /usr/local/src/aether/migrations
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
