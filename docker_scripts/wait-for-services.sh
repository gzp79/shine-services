#!/bin/bash

# Function to check if a service is ready
wait_for_service() {
  local service_host="$1"
  local service_port="$2"
  local max_retries=30
  local retries=0

  while [ $retries -lt $max_retries ]; do
    echo "Checking $service_host:$service_port ..."
    if nc -z "$service_host" "$service_port"; then
      echo "$service_host:$service_port is available."
      return 0
    else
      echo "Waiting for $service_host:$service_port to be available..."
      sleep 1
    fi
    retries=$((retries + 1))
  done

  echo "Error: $service_host:$service_port is not available after waiting for $max_retries seconds."
  return 1
}

# Check if WAIT_FOR_SERVICES environment variable is set
if [ -z "$WAIT_FOR_SERVICES" ]; then
  echo "Error: Environment variable WAIT_FOR_SERVICES is not set."
  exit 1
fi

# Convert the comma-separated list of services into an array
IFS=',' read -r -a services_array <<< "$WAIT_FOR_SERVICES"

# Wait for each service in the array
for service in "${services_array[@]}"; do
  IFS=':' read -r -a service_info <<< "$service"
  if [ "${#service_info[@]}" -ne 2 ]; then
    echo "Error: Invalid format for service '$service'. Use 'host:port' format."
    exit 1
  fi

  wait_for_service "${service_info[0]}" "${service_info[1]}" || exit 1
done
