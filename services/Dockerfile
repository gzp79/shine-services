# syntax=docker/dockerfile:1-labs

FROM rust:bookworm AS build

RUN USER=root

RUN rustup component add rustfmt

RUN rustc --version \
    && cargo --version

# Create a dummy project to cache dependencies as a separate layer
WORKDIR /shine

ENV ALL_BINS="services/identity services/builder client"
ENV ALL_LIBS="crates/shine-test-macros crates/shine-test crates/shine-core crates/shine-forge crates/shine-game crates/shine-infra-macros crates/shine-infra"
ENV SERVICE_PROJECTS="shine-test-macros shine-test shine-core shine-forge shine-infra-macros shine-infra shine-identity shine-builder"

# copy all cargo.toml files
COPY --parents ./**/Cargo.toml ./
# create empty main projects
RUN for bin in $ALL_BINS; do \
        mkdir -p ./$bin/src; \
        echo "fn main() {}" > ./$bin/src/main.rs; \
    done
# create empty lib projects
RUN for lib in $ALL_LIBS; do \
        mkdir -p ./$lib/src; \
        touch ./$lib/src/lib.rs; \
    done

COPY ./rustfmt.toml ./
COPY ./clippy.toml ./
COPY ./Cargo.toml ./
COPY ./Cargo.lock ./

RUN for proj in $SERVICE_PROJECTS; do PROJECTS="$PROJECTS -p $proj"; done \
    && cargo build --release $PROJECTS

RUN rm -rf ./crates \
    && rm -rf ./services \
    && rm -rf ./client \
    && rm -rf ./target/release/deps/libshine* \
    && rm -rf ./target/release/deps/shine*

# Copy the actual source code
COPY ./crates ./crates
COPY ./services ./services
COPY ./client ./client

RUN cargo fmt --check

ENV RUST_BACKTRACE=1
ENV RUST_LOG="debug"
ENV SHINE_TEST_REDIS_CNS="redis://redis.mockbox.com:6379"
ENV SHINE_TEST_PG_CNS="postgres://username:password@postgres.mockbox.com:5432/database-test?sslmode=disable"

# build the final project  
RUN for proj in $SERVICE_PROJECTS; do PROJECTS="$PROJECTS -p $proj"; done \
  && cargo build --release $PROJECTS
# running tests
RUN for proj in $SERVICE_PROJECTS; do PROJECTS="$PROJECTS -p $proj"; done \
  && cargo test --release $PROJECTS 

#######################################################
FROM nginx:bookworm AS base

# add ca-certs required for many tools
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates supervisor \
    && mkdir -p /var/log/supervisor

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

EXPOSE 80 443

CMD ["/usr/bin/supervisord"]

#######################################################
FROM base AS test

WORKDIR /app
COPY ./certs/scytta.crt ./certs/scytta.crt
COPY ./certs/scytta.key ./certs/scytta.key
COPY ./services/docker/nginx.test.conf /etc/nginx/nginx-shine-server.conf

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
COPY ./services/docker/nginx.prod.conf /etc/nginx/nginx-shine-server.conf

WORKDIR /app/services
COPY ./services/server_config.json ./

WORKDIR /app/services/identity
COPY ./services/identity/server_config.prod.json ./

WORKDIR /app/services/builder
COPY ./services/builder/server_config.prod.json ./

ARG ENVIRONMENT=prod
ENV ENVIRONMENT=$ENVIRONMENT
