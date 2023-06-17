FROM rust:bullseye as build

RUN USER=root
WORKDIR /server

COPY ./shine-service-rs ./shine-service-rs
COPY ./src ./src
COPY ./sql_migrations ./sql_migrations
COPY ./sql_migrations ./sql_migrations
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

RUN cargo build --release --no-default-features

FROM debian:bullseye-slim

WORKDIR /services/identity
COPY --from=build /server/target/release/shine-identity ./
COPY ./server_config.cloud.json ./server_config.json
COPY ./tera_templates ./tera_templates

EXPOSE 80
CMD ["/services/identity/shine-identity"]
