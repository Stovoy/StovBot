#!/bin/bash -eu
# For running stovbot from a Docker container.

set -o pipefail

docker rm -f stovbot 2>/dev/null || true
docker run -d -it -v $(pwd):/app \
    -p 8000:8000 \
    --name stovbot \
    stovbot \
    --twitch --discord --server
docker logs -f stovbot
