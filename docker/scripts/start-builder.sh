#!/bin/bash

export AZURE_TENANT_ID="${IDENTITY_TENANT_ID}"
export AZURE_CLIENT_ID="${IDENTITY_CLIENT_ID}"
export AZURE_CLIENT_SECRET="${IDENTITY_CLIENT_SECRET}"

export RUST_LOG="INFO,shine_core=TRACE,shine_identity=TRACE,shine_builder=TRACE"

echo "Starting service for stage: ${ENVIRONMENT}..."
cd ./services/builder
./shine-builder ${ENVIRONMENT}
