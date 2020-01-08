#!/bin/bash -eu
# For efficient testing.

set -o pipefail

cargo build --bin script_engine --no-default-features
cargo test --no-default-features
