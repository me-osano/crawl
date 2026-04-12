#!/usr/bin/env bash
set -euo pipefail

PURGE=false
PASSTHROUGH_ARGS=()

for arg in "$@"; do
    if [ "$arg" = "--purge" ]; then
        PURGE=true
    else
        PASSTHROUGH_ARGS+=("$arg")
    fi
done

if ! command -v pacman >/dev/null 2>&1; then
    echo "pacman not found. PKGBUILD uninstall is supported on Arch-based systems only." >&2
    exit 1
fi

if command -v systemctl >/dev/null 2>&1; then
    echo "==> Disabling crawl user service (if active)"
    systemctl --user disable --now crawl >/dev/null 2>&1 || true
fi

echo "==> Removing crawl package"
pacman -Rns crawl "${PASSTHROUGH_ARGS[@]}"

if [ "$PURGE" = true ]; then
    echo "==> Removing user config and local repo"
    rm -rf "$HOME/.config/crawl" "$HOME/.local/share/crawl"
fi
