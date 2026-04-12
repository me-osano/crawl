#!/usr/bin/env bash
set -euo pipefail

SOURCE_REPO="https://github.com/me-osano/crawl"
DRY_RUN=false
PASSTHROUGH_ARGS=()

for arg in "$@"; do
    if [ "$arg" = "--dry-run" ]; then
        DRY_RUN=true
    else
        PASSTHROUGH_ARGS+=("$arg")
    fi
done

if ! command -v pacman >/dev/null 2>&1; then
    echo "pacman not found. PKGBUILD update is supported on Arch-based systems only." >&2
    exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
    echo "curl not found. Install curl before running this script." >&2
    exit 1
fi

if ! command -v git >/dev/null 2>&1; then
    echo "git not found. Install git before running this script." >&2
    exit 1
fi

if ! command -v makepkg >/dev/null 2>&1; then
    echo "Installing base-devel for makepkg" >&2
    sudo pacman -S --needed base-devel
fi

echo "==> Resolving latest crawl release"
LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "$SOURCE_REPO/releases/latest")
TAG="${LATEST_URL##*/}"

if [ -z "$TAG" ] || [ "$TAG" = "latest" ]; then
    echo "Failed to resolve latest release tag." >&2
    exit 1
fi

if [ "$DRY_RUN" = true ]; then
    echo "$TAG"
    exit 0
fi

WORK_DIR=$(mktemp -d)
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

echo "==> Fetching crawl $TAG"
git clone --depth 1 --branch "$TAG" "$SOURCE_REPO" "$WORK_DIR"

PKG_DIR="$WORK_DIR/pkg"
if [ ! -d "$PKG_DIR" ]; then
    echo "PKGBUILD directory not found: $PKG_DIR" >&2
    exit 1
fi

cd "$PKG_DIR"

echo "==> Building and installing crawl via PKGBUILD"
makepkg -si "${PASSTHROUGH_ARGS[@]}"
