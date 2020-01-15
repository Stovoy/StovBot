#!/bin/bash -eu
# For building and pushing stovbot.

set -o pipefail

VERSION=$(cat Cargo.toml | grep '^version' | awk -F'=' '{print $2}' | sed 's/[ "]//g')
DOCKER_BUILDKIT=1 docker build -t stovoy/stovbot:${VERSION} .
docker --config .docker push stovoy/stovbot:${VERSION}
