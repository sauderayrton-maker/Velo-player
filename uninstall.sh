#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
DIM='\033[2m'
BLUE='\033[34m'
GREEN='\033[32m'
RESET='\033[0m'

banner() { echo -e "\n${BOLD}${BLUE}  $*${RESET}\n"; }
ok()     { echo -e "  ${GREEN}✓${RESET}  $*"; }
info()   { echo -e "  ${DIM}→${RESET}  $*"; }

PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
APPDIR="$PREFIX/share/applications"
ICONDIR="$PREFIX/share/icons/hicolor/scalable/apps"

banner "Velo Player — Uninstaller"

info "Removing files from $PREFIX..."
sudo rm -f "$BINDIR/velo-player" "$BINDIR/velo-player-uninstall"
sudo rm -f "$APPDIR/velo-player.desktop"
sudo rm -f "$ICONDIR/velo-player.svg"
sudo update-desktop-database "$APPDIR" 2>/dev/null || true
sudo gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true

ok "Velo Player removed from $PREFIX"

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/velo-player"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/velo-player"
if [[ -d "$CONFIG_DIR" || -d "$CACHE_DIR" ]]; then
    echo ""
    read -rp "  Also delete your Velo Player data — Client ID, login, recents (${CONFIG_DIR}, ${CACHE_DIR})? [y/N] " ans
    if [[ "$ans" =~ ^[Yy]$ ]]; then
        rm -rf "$CONFIG_DIR" "$CACHE_DIR"
        ok "Data removed"
    else
        info "Kept $CONFIG_DIR and $CACHE_DIR"
    fi
fi

echo ""
echo -e "${BOLD}  Velo Player has been uninstalled.${RESET}"
echo ""
