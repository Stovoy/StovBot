#!/bin/bash -eu
# For building and pushing stovbot.

set -o pipefail

VERSION=$(cat Cargo.toml | grep '^version' | awk -F'=' '{print $2}' | sed 's/[ "]//g')
docker build -t stovoy/stovbot:${VERSION} .
docker push stovoy/stovbot:${VERSION}
