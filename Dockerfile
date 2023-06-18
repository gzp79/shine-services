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
COPY ./docker_scripts ./
RUN chmod +x ./start.sh
COPY ./server_config.cloud.json ./server_config.json
COPY ./tera_templates ./tera_templates

ENV IDENTITY_TANENT_ID=
ENV IDENTITY_CLIENT_ID=
ENV IDENTITY_CLIENT_SECRET=

EXPOSE 80
CMD ["/services/identity/start.sh"]
