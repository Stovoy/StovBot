#!/bin/bash -eu
# Run checks, unit tests, and linters.

set -o pipefail

cargo check
cargo check --all-features
cargo build --bin script_engine --all-features
cargo test --all-features
cargo clippy
