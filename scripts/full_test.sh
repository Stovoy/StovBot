#!/bin/bash -eu
# For full testing.

set -o pipefail

cargo check --no-default-features
cargo check --no-default-features --features twitch
cargo check --no-default-features --features discord
cargo check --all-features
cargo build --bin script_engine --all-features
cargo test --all-features
