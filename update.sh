#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

echo "Checking for updates..."
git fetch --quiet origin
BRANCH="$(git rev-parse --abbrev-ref HEAD)"

if [[ "$(git rev-parse HEAD)" == "$(git rev-parse "origin/$BRANCH")" ]]; then
    echo "Already up to date."
    exit 0
fi

echo "Pulling latest changes..."
git merge --ff-only "origin/$BRANCH"

echo "Building Velo Player (this may take a few minutes)..."
cargo build --release

echo "Installing..."
PREFIX="${PREFIX:-/usr/local}"
if [[ -t 1 ]]; then
    sudo make -C "$REPO_DIR" install-bin PREFIX="$PREFIX"
elif command -v pkexec &>/dev/null; then
    pkexec make -C "$REPO_DIR" install-bin PREFIX="$PREFIX"
else
    echo "Need root to finish installing. Run:" >&2
    echo "  sudo make -C $REPO_DIR install-bin PREFIX=$PREFIX" >&2
    exit 1
fi

echo "Velo Player updated to $(git rev-parse --short HEAD)."
