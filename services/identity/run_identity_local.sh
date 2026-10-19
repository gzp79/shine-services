#!/bin/bash
cd "$(dirname "$0")"
# `SHINE--...` names are not valid bash identifiers, so they cannot be exported; `env` passes them as-is.
exec env \
    RUST_BACKTRACE=full \
    "SHINE--SERVICE--TLS--CERT=../../certificates/scytta.crt" \
    "SHINE--SERVICE--TLS--KEY=../../certificates/scytta.key" \
    "SHINE--SERVICE--PORT=8443" \
    "SHINE--TELEMETRY--METRICS--TYPE=otlp" \
    "SHINE--TELEMETRY--METRICS--ENDPOINT=http://localhost:4317" \
    "SHINE--TELEMETRY--TRACING--TYPE=otlp" \
    "SHINE--TELEMETRY--TRACING--ENDPOINT=http://localhost:4317" \
    "SHINE--TELEMETRY--DEFAULT_LEVEL=INFO,shine=DEBUG" \
    cargo run -p shine-identity --release -- test
