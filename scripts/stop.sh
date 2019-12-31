#!/bin/bash -eu
# For stopping stovbot.

set -o pipefail

docker rm -f stovbot 2>/dev/null || true
