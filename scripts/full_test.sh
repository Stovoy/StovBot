#!/bin/bash -eu
# For full testing.

set -o pipefail

cargo build --bin script_engine --all-features
cargo test --all-features
