#!/bin/bash -eu
# For running stovbot from a Docker container.

set -o pipefail

docker rm -f stovbot 2>/dev/null || true
docker run -d -v $(pwd)/secrets.toml:/secrets.toml --name stovbot stovbot
