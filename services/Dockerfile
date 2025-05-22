FROM rust:bookworm AS build

RUN USER=root

RUN rustup component add rustfmt

# Create a dummy project to cache dependencies as a separate layer
WORKDIR /shine
COPY ./crates/shine-test-macros/Cargo.toml ./crates/shine-test-macros/
RUN mkdir -p ./crates/shine-test-macros/src && touch ./crates/shine-test-macros/src/lib.rs

COPY ./crates/shine-test/Cargo.toml ./crates/shine-test/
RUN mkdir -p ./crates/shine-test/src && touch ./crates/shine-test/src/lib.rs

COPY ./crates/shine-core/Cargo.toml ./crates/shine-core/
RUN mkdir -p ./crates/shine-core/src && touch ./crates/shine-core/src/lib.rs

COPY ./crates/shine-infra-macros/Cargo.toml ./crates/shine-infra-macros/
RUN mkdir -p ./crates/shine-infra-macros/src && touch ./crates/shine-infra-macros/src/lib.rs

COPY ./crates/shine-infra/Cargo.toml ./crates/shine-infra/
RUN mkdir -p ./crates/shine-infra/src && touch ./crates/shine-infra/src/lib.rs

COPY ./crates/shine-game/Cargo.toml ./crates/shine-game/
RUN mkdir -p ./crates/shine-game/src && touch ./crates/shine-game/src/lib.rs

COPY ./services/identity/Cargo.toml ./services/identity/
RUN mkdir -p ./services/identity/src && echo "fn main() {}" >./services/identity/src/main.rs

COPY ./services/builder/Cargo.toml ./services/builder/
RUN mkdir -p ./services/builder/src && echo "fn main() {}" >./services/builder/src/main.rs

# client is a dummy project, but required for the workspace to build
RUN cargo new --bin client --name shine-client

COPY ./rustfmt.toml ./
COPY ./clippy.toml ./
COPY ./Cargo.toml ./
COPY ./Cargo.lock ./

RUN cargo build --release 
RUN rm -rf ./crates \
    && rm -rf ./services \
    && rm -f ./target/release/deps/libshine* \
    && rm -f ./target/release/deps/shine*

# Copy the actual source code
COPY ./crates ./crates
COPY ./services ./services
# Skipping client, the dummy project is enough 
# COPY ./client ./client  

RUN cargo fmt --check

ENV RUST_BACKTRACE=1
ENV SHINE_TEST_REDIS_CNS="redis://redis.mockbox.com:6379"
ENV SHINE_TEST_PG_CNS="postgres://username:password@postgres.mockbox.com:5432/database-test?sslmode=disable"
RUN cargo test --release

RUN cargo build --release

#######################################################
FROM nginx:bookworm AS base

# add ca-certs required for many tools
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates supervisor \
    && mkdir -p /var/log/supervisor

COPY ./docker/supervisord.conf /etc/supervisor/conf.d/supervisor.conf
COPY ./docker/nginx.conf /etc/nginx/nginx.conf

WORKDIR /app
COPY ./docker/scripts/ ./
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

ENV IDENTITY_TENANT_ID=
ENV IDENTITY_CLIENT_ID=
ENV IDENTITY_CLIENT_SECRET=
ENV WAIT_FOR_SERVICES=

EXPOSE 80 443

CMD ["/usr/bin/supervisord"]

#######################################################
FROM base AS test

WORKDIR /app
COPY ./certs/test.crt ./certs/test.crt
COPY ./certs/test.key ./certs/test.key
COPY ./docker/nginx.test.conf /etc/nginx/nginx-shine-server.conf

WORKDIR /app/services
COPY ./services/server_config_test.json ./

WORKDIR /app/services/identity
COPY ./services/identity/server_config.test.json ./

WORKDIR /app/services/builder
COPY ./services/builder/server_config.test.json ./

ARG ENVIRONMENT=test
ENV ENVIRONMENT=$ENVIRONMENT

#######################################################
FROM base AS prod

WORKDIR /app
COPY ./docker/nginx.prod.conf /etc/nginx/nginx-shine-server.conf

WORKDIR /app/services
COPY ./services/server_config.json ./

WORKDIR /app/services/identity
COPY ./services/identity/server_config.prod.json ./

WORKDIR /app/services/builder
COPY ./services/builder/server_config.prod.json ./

ARG ENVIRONMENT=prod
ENV ENVIRONMENT=$ENVIRONMENT