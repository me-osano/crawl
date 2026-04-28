#!/usr/bin/env bash
set -euo pipefail

BUILD_DIR="${BUILD_DIR:-/usr/local/share/crawl}"
REPO_URL="https://github.com/me-osano/crawl.git"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; } 
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

BANNER="
    █████████  ███████████     █████████   █████   ███   █████ █████      
  ███░░░░░███░░███░░░░░███   ███░░░░░███ ░░███   ░███  ░░███ ░░███       
 ███     ░░░  ░███    ░███  ░███    ░███  ░███   ░███   ░███  ░███       
░███          ░██████████   ░███████████  ░███   ░███   ░███  ░███        
░███          ░███░░░░░███  ░███░░░░░███  ░░███  █████  ███   ░███       
░░███     ███ ░███    ░███  ░███    ░███   ░░░█████░█████░    ░███      █
░░█████████  █████   █████ █████   █████    ░░███ ░░███      ███████████ 
░░░░░░░░░  ░░░░░   ░░░░░ ░░░░░   ░░░░░      ░░░   ░░░      ░░░░░░░░░░░  
"

# ======= Clone source ===================
setup_temp_source() {
    TMP_DIR=$(mktemp -d)
    export TMP_DIR
    
    log_info "Cloning Crawl repo..."
    git clone --depth 1 "$REPO_URL" "$TMP_DIR" || {
        log_error "Failed to clone repository"
        exit 1
    }
}

cleanup() {
    [[ -n "${TMP_DIR:-}" ]] && rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# ========== Building crawl daemon ==============
install_daemon() {
    log_info "Installing crawl daemon..."

    cd "$TMP_DIR"

    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR="$BUILD_DIR/target"

    log_step "Building crawl (this may take a few minutes)..."
    if ! cargo build --release --workspace --bins 2>&1; then
        log_error "Build failed"
        exit 1
    fi

    if [[ ! -f "$CARGO_TARGET_DIR/release/crawl-daemon" ]]; then
        log_error "Build failed - daemon not found"
        exit 1
    fi

    log_info "Installing crawl-daemon binaries..."
    sudo install -Dm755 "$CARGO_TARGET_DIR/release/crawl-daemon" /usr/local/bin/crawl-daemon
    sudo install -Dm755 "$CARGO_TARGET_DIR/release/crawl" /usr/local/bin/crawl
    
    log_info "Copying default config"
    mkdir -p ~/.config/crawl
    cp assets/config/crawl.toml ~/.config/crawl/config.toml
    cp assets/config/crawl.toml /etc/crawl/config.toml
    
    log_info "Setting up crawl service"
    sudo cp assets/etc/systemd.service /etc/systemd/system/crawl.service
    sudo systemctl daemon-reexec
    sudo systemctl daemon-reload
    
    sudo systemctl enable crawl
    sudo systemctl start crawl
    
    log_info "Setting up brightness permissions"
    sudo cp assets/etc/90-crawl-backlight.rules /etc/udev/rules.d/90-crawl-backlight.rules
    sudo usermod -aG video $USER

    log_success "crawl daemon installed and running!"
}

echo -e "${BANNER}"
echo -e "  Crawl Installation"
echo "========================================="
echo

setup_temp_source
install_daemon

echo
echo "========================================="
echo -e "  ${GREEN}Installation Complete!${NC}"
echo "========================================="
echo