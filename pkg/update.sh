#!/usr/bin/env bash
set -euo pipefail

SOURCE_REPO="https://github.com/me-osano/crawl"
INSTALL_DIR="$HOME/.local/share/crawl"
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

if ! command -v git >/dev/null 2>&1; then
    echo "git not found. Install git before running this script." >&2
    exit 1
fi

if ! command -v makepkg >/dev/null 2>&1; then
    echo "Installing base-devel for makepkg" >&2
    sudo pacman -S --needed base-devel
fi

if [ -d "$INSTALL_DIR/.git" ]; then
    echo "==> Updating existing repo in $INSTALL_DIR"
    if [ -n "$(git -C "$INSTALL_DIR" status --porcelain)" ]; then
        echo "Install path has uncommitted changes: $INSTALL_DIR" >&2
        exit 1
    fi
elif [ -e "$INSTALL_DIR" ]; then
    echo "Install path exists but is not a git repo: $INSTALL_DIR" >&2
    exit 1
else
    echo "==> Cloning crawl into $INSTALL_DIR"
    git clone "$SOURCE_REPO" "$INSTALL_DIR"
fi

echo "==> Resolving latest crawl release"
git -C "$INSTALL_DIR" fetch --tags --quiet
TAG=$(git -C "$INSTALL_DIR" tag --list "v*" --sort=-v:refname | head -n 1)

if [ -z "$TAG" ] || [ "$TAG" = "latest" ]; then
    echo "Failed to resolve latest release tag." >&2
    exit 1
fi

if [ "$DRY_RUN" = true ]; then
    echo "$TAG"
    exit 0
fi

INSTALLED_VERSION=$(pacman -Qi crawl 2>/dev/null | awk -F': +' '/^Version/{print $2}')
TAG_VERSION="${TAG#v}"
INSTALLED_BASE="${INSTALLED_VERSION%%-*}"
if [ -n "$INSTALLED_BASE" ] && [ "$INSTALLED_BASE" = "$TAG_VERSION" ]; then
    echo "==> Already on latest release ($TAG)"
    exit 0
fi

if [ -n "$INSTALLED_VERSION" ]; then
    echo "==> Installed version: $INSTALLED_VERSION"
fi
echo "==> Target release: $TAG"

CURRENT_REF=$(git -C "$INSTALL_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || true)
DEFAULT_REF=$(git -C "$INSTALL_DIR" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null || true)
DEFAULT_BRANCH="${DEFAULT_REF#origin/}"
if [ -z "$DEFAULT_BRANCH" ]; then
    DEFAULT_BRANCH="master"
fi

RESTORE_REF=""
if [ -n "$CURRENT_REF" ] && [ "$CURRENT_REF" != "HEAD" ]; then
    if [ "$CURRENT_REF" != "$DEFAULT_BRANCH" ]; then
        RESTORE_REF="$CURRENT_REF"
        git -C "$INSTALL_DIR" checkout -q "$DEFAULT_BRANCH"
    fi
    git -C "$INSTALL_DIR" pull --ff-only
else
    git -C "$INSTALL_DIR" checkout -q "$DEFAULT_BRANCH"
    git -C "$INSTALL_DIR" pull --ff-only
fi

PKG_DIR="$INSTALL_DIR/pkg"
if [ ! -d "$PKG_DIR" ]; then
    echo "PKGBUILD directory not found: $PKG_DIR" >&2
    exit 1
fi

cd "$PKG_DIR"

echo "==> Building and installing crawl via PKGBUILD"
makepkg -si "${PASSTHROUGH_ARGS[@]}"

if command -v systemctl >/dev/null 2>&1; then
    echo "==> Reloading crawl user service"
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    systemctl --user restart crawl >/dev/null 2>&1 || true
fi

if command -v crawl >/dev/null 2>&1; then
    echo "==> Restarting crawl daemon"
    crawl daemon --restart >/dev/null 2>&1 || true
fi

if [ -n "$RESTORE_REF" ]; then
    echo "==> Restoring repo to $RESTORE_REF"
    git -C "$INSTALL_DIR" checkout -q "$RESTORE_REF"
fi
