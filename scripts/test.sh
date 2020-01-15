#!/bin/bash -eu
# Run checks, unit tests, and linters.

set -o pipefail

cargo build --bin script_engine
cargo test
cargo clippy
