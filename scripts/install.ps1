# rustwerk installer for Windows.
#
# Usage:
#   irm https://raw.githubusercontent.com/breki/rustwerk/main/scripts/install.ps1 | iex
#
# Environment variables (set before piping to iex):
#   $env:RUSTWERK_VERSION       Version tag (default: latest). Example: v0.40.0
#   $env:RUSTWERK_INSTALL_DIR   Install dir (default: %LOCALAPPDATA%\Programs\rustwerk\bin)
#   $env:RUSTWERK_MODIFY_PATH   Set to 1 to auto-append install dir to user PATH.
#                               Default: 0 (print hint only).

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 defaults to TLS 1.0/1.1; GitHub requires 1.2.
# Harmless on pwsh 7 where 1.2 is already the default.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = 'breki/rustwerk'
$Bin = 'rustwerk.exe'

$InstallDir = if ($env:RUSTWERK_INSTALL_DIR) {
    $env:RUSTWERK_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA 'Programs\rustwerk\bin'
}

# Reject values that would corrupt the user's persistent PATH when appended.
if ($InstallDir -match ';') {
    throw "RUSTWERK_INSTALL_DIR must not contain ';'"
}

$Version = if ($env:RUSTWERK_VERSION) { $env:RUSTWERK_VERSION } else { 'latest' }
$ModifyPath = $env:RUSTWERK_MODIFY_PATH -eq '1'

# Detect architecture. A 32-bit PowerShell host on 64-bit Windows reports
# PROCESSOR_ARCHITECTURE as 'x86', so consult PROCESSOR_ARCHITEW6432 first.
$rawArch = if ($env:PROCESSOR_ARCHITEW6432) {
    $env:PROCESSOR_ARCHITEW6432
} else {
    $env:PROCESSOR_ARCHITECTURE
}
$arch = switch ($rawArch) {
    'AMD64' { 'x86_64-pc-windows-msvc' }
    default { throw "unsupported architecture: $rawArch" }
}

function Resolve-LatestTag {
    # Try the API first, fall back to the releases/latest redirect if the
    # caller is rate-limited (60 req/hr/IP, unauthenticated).
    try {
        $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
        if ($release.tag_name) { return $release.tag_name }
    } catch {
        Write-Host "API lookup failed ($($_.Exception.Message)); falling back to redirect..."
    }
    $resp = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" `
        -MaximumRedirection 0 -ErrorAction SilentlyContinue -UseBasicParsing
    $location = $resp.Headers.Location
    if ($location -and $location -match '/tag/([^/]+)$') { return $Matches[1] }
    throw 'could not resolve latest version'
}

if ($Version -eq 'latest') {
    Write-Host 'resolving latest release...'
    $Version = Resolve-LatestTag
}
if ($Version -notlike 'v*') { $Version = "v$Version" }

$archiveName = "rustwerk-$Version-$arch.zip"
$baseUrl = "https://github.com/$Repo/releases/download/$Version"
$archiveUrl = "$baseUrl/$archiveName"
$sumsUrl = "$baseUrl/SHA256SUMS"

$tmp = $null
try {
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("rustwerk-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    $extractDir = Join-Path $tmp 'extract'
    New-Item -ItemType Directory -Path $extractDir -Force | Out-Null

    Write-Host "downloading $archiveName..."
    $archivePath = Join-Path $tmp $archiveName
    Invoke-WebRequest -Uri $archiveUrl -OutFile $archivePath -UseBasicParsing

    Write-Host 'downloading SHA256SUMS...'
    $sumsPath = Join-Path $tmp 'SHA256SUMS'
    Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath -UseBasicParsing

    # Verify checksum. SHA256SUMS lines are "<hash>  <name>" or "<hash> *<name>";
    # split on whitespace and match the second field exactly.
    $expected = $null
    foreach ($line in Get-Content $sumsPath) {
        $parts = $line -split '\s+', 2
        if ($parts.Count -eq 2) {
            $name = $parts[1].TrimStart('*')
            if ($name -eq $archiveName) { $expected = $parts[0]; break }
        }
    }
    if (-not $expected) { throw "no checksum entry for $archiveName" }

    $actual = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLower()
    if ($expected.ToLower() -ne $actual) {
        throw "checksum mismatch (expected $expected, got $actual)"
    }
    Write-Host 'checksum OK'

    # Extract and install. Expected layout: <extractDir>\rustwerk-<ver>-<arch>\rustwerk.exe
    # Fall back to a recursive search if the layout ever changes.
    Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force
    $staging = "rustwerk-$Version-$arch"
    $binSrc = Join-Path $extractDir "$staging\$Bin"
    if (-not (Test-Path $binSrc)) {
        $binSrc = Get-ChildItem -Path $extractDir -Recurse -Filter $Bin -File |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $binSrc) { throw "binary not found in archive" }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $binDst = Join-Path $InstallDir $Bin
    # Remove any existing entry first so symlinks in InstallDir are replaced,
    # not followed.
    if (Test-Path $binDst) { Remove-Item -Force -LiteralPath $binDst }
    Copy-Item -Path $binSrc -Destination $binDst -Force

    Write-Host ''
    Write-Host "installed rustwerk $Version to $binDst"

    # PATH handling: print a hint by default; only mutate persistent PATH when
    # explicitly opted in, to match install.sh's contract.
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = if ($userPath) { $userPath -split ';' } else { @() }
    if ($entries -notcontains $InstallDir) {
        if ($ModifyPath) {
            $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
            [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
            Write-Host ''
            Write-Host "added $InstallDir to user PATH"
            Write-Host 'open a new terminal for the PATH change to take effect.'
        } else {
            Write-Host ''
            Write-Host "note: $InstallDir is not on PATH. Add it via:"
            Write-Host "    [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$InstallDir`", 'User')"
            Write-Host "or re-run this installer with `$env:RUSTWERK_MODIFY_PATH=1."
        }
    }
}
finally {
    if ($tmp -and (Test-Path $tmp)) {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}
