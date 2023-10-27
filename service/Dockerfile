FROM rust:bullseye as build

RUN USER=root

RUN apt update \
    && apt install -y jq \
    && rustup component add rustfmt

# Create a layer of the dependencies (including submodules)
RUN cargo new --bin shine-identity
WORKDIR /shine-identity
COPY ./shine-service-rs ./shine-service-rs
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release --no-default-features
RUN rm -rf ./src \
    && rm -f ./target/release/deps/shine_identity* \
    && rm -f ./target/release/deps/shine-identity* \
    && rm -f ./target/release/shine_identity* \
    && rm -f ./target/release/shine-identity*

# Prepare for build
WORKDIR /shine-identity
COPY ./src ./src
COPY ./sql_migrations ./sql_migrations
COPY ./rustfmt.toml ./

# Perform quality checks - code format
RUN cargo fmt --check

# Perform quality checks - unit tests
ENV RUST_BACKTRACE=1
ENV SHINE_TEST_REDIS_CNS="redis://redis.mockbox.com:6379"
ENV SHINE_TEST_PG_CNS="postgres://username:password@postgres.mockbox.com:5432/database-name?sslmode=disable"
RUN cargo test --release

# Perform the build (only if checks are ok)
RUN cargo build --release --no-default-features 


#######################################################
FROM debian:bullseye-slim as base

# add ca-certs required for many tools
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates

WORKDIR /services/identity
COPY --from=build /shine-identity/target/release/shine-identity ./
COPY ./docker_scripts ./
COPY ./server_config.json ./
COPY ./server_version.json ./
COPY ./tera_templates ./tera_templates

ENV IDENTITY_TENANT_ID=
ENV IDENTITY_CLIENT_ID=
ENV IDENTITY_CLIENT_SECRET=
ENV WAIT_FOR_SERVICES=

EXPOSE 80
RUN chmod +x ./start.sh

#######################################################
FROM base as test

RUN apt install -y --no-install-recommends netcat

WORKDIR /services/identity
COPY ./certs/test.crt ./certs/test.crt
COPY ./certs/test.key ./certs/test.key
COPY ./server_config.test.json ./

CMD ["/services/identity/start.sh", "test"]

#######################################################
# make sure prod is the last stage bc of the default deployment policy
FROM base as prod

WORKDIR /services/identity
COPY ./server_config.prod.json ./

CMD ["/services/identity/start.sh","prod"]
