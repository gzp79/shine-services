# syntax=docker/dockerfile:1-labs

FROM rust:bookworm AS build_base

RUN USER=root

RUN rustup component add rustfmt
RUN cargo install cargo-chef

RUN rustc --version \
    && cargo --version

#######################################################
FROM build_base AS planner

WORKDIR /shine
COPY . .

RUN cargo fmt --check
RUN cargo chef prepare --recipe-path recipe.json

#######################################################
FROM build_base AS build

# Dependency layer
WORKDIR /shine
COPY --from=planner /shine/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build layer
COPY . .
RUN cargo build --release $PROJECTS --workspace --locked

ENV RUST_BACKTRACE=1
ENV RUST_LOG="none"
ENV SHINE_TEST_TOXIPROXY_URL="http://toxiproxy.mockbox.foo:8474"
ENV SHINE_TEST_REDIS_CNS="redis://redis.mockbox.foo:6379"
ENV SHINE_TEST_REDIS_CNS_DIRECT="redis://redis-direct.mockbox.foo:6379"
ENV SHINE_TEST_PG_CNS="postgres://username:password@postgres.mockbox.foo:5432/database-test?sslmode=disable"
ENV SHINE_TEST_PG_CNS_DIRECT="postgres://username:password@postgres-direct.mockbox.foo:5432/database-test?sslmode=disable"

RUN cargo test --workspace --locked -- --test-threads=1

#######################################################
FROM nginx:bookworm AS base

# add ca-certs required for many tools
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates supervisor libcap2-bin \
    && rm -rf /var/lib/apt/lists/*

# allow the unprivileged nginx user to bind ports 80/443 without running as root
RUN setcap 'cap_net_bind_service=+ep' /usr/sbin/nginx

# --- install otelcol ---
RUN curl -L https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v0.108.0/otelcol_0.108.0_linux_amd64.tar.gz \
    | tar -xz -C /usr/local/bin
COPY ./services/docker/otel-collector-config.yaml /etc/otelcol/config.yaml


COPY ./services/docker/supervisord.conf /etc/supervisor/conf.d/supervisor.conf
COPY ./services/docker/nginx.conf /etc/nginx/nginx.conf

WORKDIR /app
COPY ./services/docker/scripts/ ./
RUN  chmod +x ./wait-for-services.sh \
    && chmod +x ./start-identity.sh \
    && chmod +x ./start-builder.sh

WORKDIR /app/services/identity
COPY --from=build /shine/target/release/shine-identity ./
COPY --from=build /shine/services/identity/tera_templates ./tera_templates
COPY ./services/server_version.json ./

WORKDIR /app/services/builder
COPY --from=build /shine/target/release/shine-builder ./
COPY ./services/server_version.json ./

# run the whole process tree (supervisord, nginx, otelcol, both services) as the unprivileged nginx user
RUN mkdir -p /var/cache/nginx /var/log/nginx \
    && chown -R nginx:nginx /var/cache/nginx /var/log/nginx /app /etc/otelcol

EXPOSE 80 443

USER nginx

CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisor.conf"]

#######################################################
FROM base AS test

WORKDIR /app
COPY ./certificates/scytta.crt ./certificates/scytta.crt
COPY ./certificates/scytta.key ./certificates/scytta.key
COPY ./services/docker/nginx.test.conf /etc/nginx/nginx-shine-server.conf

WORKDIR /app/services
COPY ./services/config/common.mock.json ./services/config/common.test.json ./config/
COPY ./services/identity/config/mock.json ./identity/config/
COPY ./services/identity/server_config.test.json ./identity
COPY ./services/builder/config/mock.json ./builder/config/
COPY ./services/builder/server_config.test.json ./builder

# Some build time validation 
RUN nginx -t -c /etc/nginx/nginx.conf

ARG ENVIRONMENT=test
ENV ENVIRONMENT=$ENVIRONMENT

#######################################################
FROM base AS prod

WORKDIR /app
COPY ./services/docker/nginx.prod.conf /etc/nginx/nginx-shine-server.conf

WORKDIR /app/services
COPY ./services/config/common.prod.json ./config/
COPY ./services/identity/server_config.prod.json ./identity
COPY ./services/builder/server_config.prod.json ./builder

# Some build time validation 
RUN nginx -t -c /etc/nginx/nginx.conf

ARG ENVIRONMENT=prod
ENV ENVIRONMENT=$ENVIRONMENT
