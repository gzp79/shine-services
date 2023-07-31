FROM rust:bullseye as build

RUN USER=root
WORKDIR /server

COPY ./shine-service-rs ./shine-service-rs
COPY ./src ./src
COPY ./sql_migrations ./sql_migrations
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

RUN cargo build --release --no-default-features

#######################################################
FROM debian:bullseye-slim as prod

# add ca-certs required for many tools
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates

WORKDIR /services/identity
COPY --from=build /server/target/release/shine-identity ./
COPY ./docker_scripts ./
COPY ./server_config.json ./
COPY ./tera_templates ./tera_templates

ENV IDENTITY_TENANT_ID=
ENV IDENTITY_CLIENT_ID=
ENV IDENTITY_CLIENT_SECRET=
ENV WAIT_FOR_SERVICES=

EXPOSE 80
RUN chmod +x ./start.sh

CMD ["/services/identity/start.sh"]

#######################################################
FROM prod as test

RUN apt install -y --no-install-recommends netcat

WORKDIR /services/identity
COPY ./server_config.test.json ./server_config.json

#######################################################
FROM prod
