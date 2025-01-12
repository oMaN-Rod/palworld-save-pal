#!/bin/bash

# Build and Run Script for PALWorld Save Pal

# Exit immediately if a command exits with a non-zero status
set -e

# Navigate to the script's directory
cd "$(dirname "$0")"

# Get IP address (Linux/macOS variants)
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    IP_ADDRESS=$(ipconfig getifaddr en0)
else
    # Linux
    IP_ADDRESS=$(hostname -I | awk '{print $1}')
fi

echo "Using IP Address: $IP_ADDRESS"

# Build and run docker compose
docker compose build --build-arg PUBLIC_WS_URL=$IP_ADDRESS
docker compose up -d

echo "Build and deployment completed successfully."
