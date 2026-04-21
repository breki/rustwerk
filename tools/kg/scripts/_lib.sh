#!/usr/bin/env bash
# Shared helpers for kg-* authoring scripts (kg-new, kg-validate,
# kg-stats). Sourced, not executed.
#
# Build and serve no longer live here — they go through
# `cargo xtask kg {build,serve}`.
set -euo pipefail

# Resolve repo root from this script's location (tools/kg/scripts/).
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
KG_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd -- "${KG_ROOT}/../.." && pwd)"

KG_KNOWLEDGE="${REPO_ROOT}/knowledge"
