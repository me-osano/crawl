#!/usr/bin/env bash
set -euo pipefail

SOURCE_REPO="https://github.com/me-osano/crawl"
INSTALL_DIR="$HOME/.local/share/crawl"

if ! command -v pacman >/dev/null 2>&1; then
    echo "pacman not found. PKGBUILD installation is supported on Arch-based systems only." >&2
    exit 1
fi

if ! command -v git >/dev/null 2>&1; then
    echo "Installing git" >&2
    sudo pacman -S --needed git
fi

if ! command -v makepkg >/dev/null 2>&1; then
    echo "Installing base-devel for makepkg" >&2
    sudo pacman -S --needed base-devel
fi

if [ -d "$INSTALL_DIR/.git" ]; then
    echo "==> Updating existing repo in $INSTALL_DIR"
    git -C "$INSTALL_DIR" pull --ff-only
elif [ -e "$INSTALL_DIR" ]; then
    echo "Install path exists but is not a git repo: $INSTALL_DIR" >&2
    exit 1
else
    echo "==> Cloning crawl into $INSTALL_DIR"
    git clone "$SOURCE_REPO" "$INSTALL_DIR"
fi

PKG_DIR="$INSTALL_DIR/pkg"
if [ ! -d "$PKG_DIR" ]; then
    echo "PKGBUILD directory not found: $PKG_DIR" >&2
    exit 1
fi

cd "$PKG_DIR"

echo "==> Building and installing crawl via PKGBUILD"
makepkg -si "$@"
