#!/bin/bash

set -e

cd "$(dirname "$0")/.."

if [[ "$OSTYPE" == "darwin"* ]]; then
    IP_ADDRESS=$(ipconfig getifaddr en0)
else
    IP_ADDRESS=$(hostname -I | awk '{print $1}')
fi

echo "Using IP Address: $IP_ADDRESS"

# The base compose file pulls the prebuilt image; the build override rebuilds
# it locally with this machine's IP baked into the UI's WebSocket URL.
export PUBLIC_WS_URL="${IP_ADDRESS}:5174/ws"
docker compose -f docker-compose.yml -f docker-compose.build.yml up -d --build

echo "Build and deployment completed successfully."
