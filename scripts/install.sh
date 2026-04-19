#!/usr/bin/env sh
# rustwerk installer for Linux and macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/breki/rustwerk/main/scripts/install.sh | sh
#
# Environment variables:
#   RUSTWERK_VERSION       Version tag to install (default: latest). Example: v0.40.0
#   RUSTWERK_INSTALL_DIR   Install directory (default: $HOME/.local/bin)

set -eu

REPO="breki/rustwerk"
BIN="rustwerk"
INSTALL_DIR="${RUSTWERK_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${RUSTWERK_VERSION:-latest}"

err() { printf 'error: %s\n' "$1" >&2; exit 1; }
info() { printf '%s\n' "$1"; }

need() { command -v "$1" >/dev/null 2>&1 || err "missing required tool: $1"; }

need uname
need tar
need mkdir
need mv
need rm
need awk

# Download helpers. Prefer curl, fall back to wget with TLS/HTTPS enforcement.
if command -v curl >/dev/null 2>&1; then
    dl_to() { curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$1" "$2"; }
    # Print response headers only (used for redirect-following fallback).
    dl_headers() { curl --proto '=https' --tlsv1.2 -fsSLI "$1"; }
elif command -v wget >/dev/null 2>&1; then
    dl_to() { wget -q --https-only --tries=3 -O "$1" "$2"; }
    dl_headers() { wget -q --https-only --tries=3 --method=HEAD -S -O /dev/null "$1" 2>&1; }
else
    err "need curl or wget"
fi

# Detect OS/arch and map to Rust target triple.
os=$(uname -s)
arch=$(uname -m)
case "$os" in
    Linux)
        case "$arch" in
            x86_64|amd64) target='x86_64-unknown-linux-gnu' ;;
            aarch64|arm64) target='aarch64-unknown-linux-gnu' ;;
            *) err "unsupported Linux arch: $arch" ;;
        esac
        ;;
    Darwin)
        case "$arch" in
            x86_64) target='x86_64-apple-darwin' ;;
            arm64) target='aarch64-apple-darwin' ;;
            *) err "unsupported macOS arch: $arch" ;;
        esac
        ;;
    *) err "unsupported OS: $os (use install.ps1 on Windows)" ;;
esac

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t rustwerk)
trap 'rm -rf "$tmp"' EXIT

# Resolve version tag. Try the GitHub API first, fall back to following the
# releases/latest redirect when rate-limited (60 req/hr/IP, unauthenticated).
if [ "$VERSION" = "latest" ]; then
    info "resolving latest release…"
    api_body="$tmp/latest.json"
    if dl_to "$api_body" "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null; then
        VERSION=$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' "$api_body" | head -n 1)
    fi
    if [ -z "${VERSION:-}" ] || [ "$VERSION" = "latest" ]; then
        info "API unavailable; falling back to redirect…"
        location=$(dl_headers "https://github.com/${REPO}/releases/latest" | \
            awk 'BEGIN{IGNORECASE=1} /^[Ll]ocation:/ {gsub(/\r/,""); print $2}' | tail -n 1)
        VERSION=$(printf '%s\n' "$location" | sed -n 's|.*/tag/\([^/]*\)$|\1|p')
    fi
    [ -n "${VERSION:-}" ] || err "could not resolve latest version"
fi

case "$VERSION" in
    v*) : ;;
    *) VERSION="v$VERSION" ;;
esac

archive="${BIN}-${VERSION}-${target}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/${VERSION}"
archive_url="${base_url}/${archive}"
sums_url="${base_url}/SHA256SUMS"

info "downloading ${archive}…"
dl_to "$tmp/$archive" "$archive_url" || err "download failed: $archive_url"

info "downloading SHA256SUMS…"
dl_to "$tmp/SHA256SUMS" "$sums_url" || err "download failed: $sums_url"

# Verify checksum. SHA256SUMS lines are "<hash>  <name>" (text mode) or
# "<hash> *<name>" (binary mode); match the second field exactly.
expected=$(awk -v name="$archive" '
    { n = $2; sub(/^\*/, "", n); if (n == name) { print $1; exit } }
' "$tmp/SHA256SUMS")
[ -n "$expected" ] || err "no checksum entry for $archive"

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp/$archive" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')
else
    err "need sha256sum or shasum to verify checksum"
fi

[ "$expected" = "$actual" ] || \
    err "checksum mismatch (expected $expected, got $actual)"
info "checksum OK"

# Extract and install. Expected layout: <tmp>/extract/<staging>/rustwerk
# Fall back to a recursive find if the layout ever changes.
mkdir -p "$tmp/extract"
tar -xzf "$tmp/$archive" -C "$tmp/extract" --no-same-owner 2>/dev/null || \
    tar -xzf "$tmp/$archive" -C "$tmp/extract"
staging="${BIN}-${VERSION}-${target}"
bin_src="$tmp/extract/$staging/$BIN"
if [ ! -f "$bin_src" ]; then
    bin_src=$(find "$tmp/extract" -name "$BIN" -type f | head -n 1)
fi
[ -n "${bin_src:-}" ] && [ -f "$bin_src" ] || err "binary not found in archive"

mkdir -p "$INSTALL_DIR"
# rm first so we replace symlinks rather than following them.
rm -f "$INSTALL_DIR/$BIN"
mv "$bin_src" "$INSTALL_DIR/$BIN"
chmod +x "$INSTALL_DIR/$BIN"

info ""
info "installed ${BIN} ${VERSION} to ${INSTALL_DIR}/${BIN}"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        info ""
        info "note: ${INSTALL_DIR} is not on PATH. Add it to your shell rc:"
        info "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
