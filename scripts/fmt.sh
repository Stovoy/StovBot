#!/bin/bash -eu
# For applying autoformatting to the code.

set -o pipefail

cargo fix --allow-dirty --allow-staged && cargo fmt
