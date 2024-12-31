#!/bin/bash

export AZURE_TENANT_ID="${IDENTITY_TENANT_ID}"
export AZURE_CLIENT_ID="${IDENTITY_CLIENT_ID}"
export AZURE_CLIENT_SECRET="${IDENTITY_CLIENT_SECRET}"

source ./wait-for-services.sh

echo "Starting service for stage: ${ENVIRONMENT}..."
cd ./services/command
./shine-command ${ENVIRONMENT}
