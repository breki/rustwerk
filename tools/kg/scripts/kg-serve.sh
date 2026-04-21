#!/usr/bin/env bash
# Serve the KG website with live reload.
# Thin wrapper around `cargo xtask kg serve`, which owns the actual
# logic (ensuring zola is available, staging content, invoking zola).
#
# Extra args pass through to `zola serve`, e.g.:
#   tools/kg/scripts/kg-serve.sh --port 8080 --open
set -euo pipefail
exec cargo xtask kg serve -- "$@"
