#!/bin/bash

set -e

cd "$(dirname "$0")/.."

if [[ "$OSTYPE" == "darwin"* ]]; then
    IP_ADDRESS=$(ipconfig getifaddr en0)
else
    IP_ADDRESS=$(hostname -I | awk '{print $1}')
fi

echo "Using IP Address: $IP_ADDRESS"

docker compose build --build-arg PUBLIC_WS_URL=${IP_ADDRESS}:7257/ws
docker compose up -d

echo "Build and deployment completed successfully."
