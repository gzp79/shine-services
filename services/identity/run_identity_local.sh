#!/bin/bash
cd "$(dirname "$0")"
export RUST_BACKTRACE=full
export SHINE--SERVICE--TLS--CERT=../../certs/scytta.crt
export SHINE--SERVICE--TLS--KEY=../../certs/scytta.key
export SHINE--SERVICE--PORT=8443
cargo run -p shine-identity --release -- test
