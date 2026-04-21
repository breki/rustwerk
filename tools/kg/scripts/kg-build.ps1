#!/usr/bin/env pwsh
# Build the KG website into tools/kg/site/public/.
# Thin wrapper around `cargo xtask kg build`, which owns the actual
# logic (ensuring zola is available, staging content, invoking zola).
#
# Requires PowerShell 7+ (pwsh).
$ErrorActionPreference = 'Stop'
& cargo xtask kg build
exit $LASTEXITCODE
