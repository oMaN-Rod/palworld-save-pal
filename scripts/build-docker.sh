#!/bin/bash

set -e

cd "$(dirname "$0")/.."

# The base compose file pulls the prebuilt image; the build override rebuilds
# it locally. The SPA dials its websocket same-origin, so no address is
# baked — the image works from any host/port it is published on.
docker compose -f docker-compose.yml -f docker-compose.build.yml up -d --build

echo "Build and deployment completed successfully."
