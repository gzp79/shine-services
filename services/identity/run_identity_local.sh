#!/bin/bash
cd "$(dirname "$0")"
export RUST_BACKTRACE=full
export SHINE--SERVICE--TLS--CERT=../../certificates/scytta.crt
export SHINE--SERVICE--TLS--KEY=../../certificates/scytta.key
export SHINE--SERVICE--PORT=8443
export SHINE--TELEMETRY--METRICS--TYPE=otlp
export SHINE--TELEMETRY--METRICS--ENDPOINT=http://localhost:4317
export SHINE--TELEMETRY--TRACING--TYPE=otlp
export SHINE--TELEMETRY--TRACING--ENDPOINT=http://localhost:4317
cargo run -p shine-identity --release -- test
