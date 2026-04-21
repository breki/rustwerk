#!/usr/bin/env bash
# Build the KG website into tools/kg/site/public/.
# Thin wrapper around `cargo xtask kg build`, which owns the actual
# logic (ensuring zola is available, staging content, invoking zola).
set -euo pipefail
exec cargo xtask kg build "$@"
