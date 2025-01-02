#!/bin/bash

# Build and Run Script for PALWorld Save Pal

# Exit immediately if a command exits with a non-zero status
set -e

# Navigate to the script's directory
cd "$(dirname "$0")"

# Run docker-compose up --build -d
echo "Starting Docker Compose..."
docker compose up --build -d

echo "Build and deployment completed successfully."
