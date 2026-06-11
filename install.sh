#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
DIM='\033[2m'
BLUE='\033[34m'
GREEN='\033[32m'
RED='\033[31m'
RESET='\033[0m'

banner() { echo -e "\n${BOLD}${BLUE}  $*${RESET}\n"; }
ok()     { echo -e "  ${GREEN}✓${RESET}  $*"; }
err()    { echo -e "  ${RED}✗${RESET}  $*"; }
info()   { echo -e "  ${DIM}→${RESET}  $*"; }

banner "Velo Player — Installer"

# ── Detect package manager ────────────────────────────────────────────────────

PM=""
if command -v pacman &>/dev/null; then
    PM="pacman"
elif command -v apt-get &>/dev/null; then
    PM="apt"
elif command -v dnf &>/dev/null; then
    PM="dnf"
fi

install_deps_pacman() {
    local pkgs=(base-devel gtk4 libadwaita alsa-lib openssl)
    info "Installing dependencies via pacman..."
    sudo pacman -S --needed --noconfirm "${pkgs[@]}"
}

install_deps_apt() {
    local pkgs=(build-essential pkg-config
                libgtk-4-dev libadwaita-1-dev libasound2-dev libssl-dev)
    info "Installing dependencies via apt..."
    sudo apt-get update -q
    sudo apt-get install -y "${pkgs[@]}"
}

install_deps_dnf() {
    local pkgs=(gcc pkg-config gtk4-devel libadwaita-devel alsa-lib-devel openssl-devel)
    info "Installing dependencies via dnf..."
    sudo dnf install -y "${pkgs[@]}"
}

# ── Rust check ────────────────────────────────────────────────────────────────

if ! command -v cargo &>/dev/null; then
    err "Rust/Cargo not found."
    info "Install Rust:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    info "Then re-run this script."
    exit 1
fi
ok "Rust $(rustc --version | cut -d' ' -f2)"

# ── System dependencies ───────────────────────────────────────────────────────

banner "System dependencies"

MISSING_HINT=""
if [[ "$PM" == "pacman" ]]; then
    install_deps_pacman
elif [[ "$PM" == "apt" ]]; then
    install_deps_apt
elif [[ "$PM" == "dnf" ]]; then
    install_deps_dnf
else
    info "Package manager not detected. Make sure these are installed:"
    info "  GTK4 (4.12+), libadwaita (1.4+), ALSA, OpenSSL"
    MISSING_HINT="true"
fi

if [[ -z "$MISSING_HINT" ]]; then
    ok "Dependencies installed"
fi

# ── Build ─────────────────────────────────────────────────────────────────────

banner "Building Velo Player"

cargo build --release

ok "Build complete"

# ── Install ───────────────────────────────────────────────────────────────────

banner "Installing"

sudo make install PREFIX=/usr/local

ok "velo-player           →  /usr/local/bin/velo-player"
ok "velo-player.desktop   →  app launcher"
ok "velo-player.svg       →  icon theme"

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}  Velo Player is installed.${RESET}"
echo ""
echo -e "  Launch from your app menu or run:  ${BOLD}velo-player${RESET}"
echo ""
echo -e "  ${DIM}First run needs a Spotify Client ID — see the README's Setup section.${RESET}"
echo ""
