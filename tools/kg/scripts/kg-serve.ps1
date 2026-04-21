#!/usr/bin/env pwsh
# Serve the KG website with live reload.
# Thin wrapper around `cargo xtask kg serve`, which owns the actual
# logic (ensuring zola is available, staging content, invoking zola).
#
# Extra args pass through to `zola serve`, e.g.:
#   pwsh tools/kg/scripts/kg-serve.ps1 --port 8080 --open
#
# Requires PowerShell 7+ (pwsh).
$ErrorActionPreference = 'Stop'
& cargo xtask kg serve -- @args
exit $LASTEXITCODE
