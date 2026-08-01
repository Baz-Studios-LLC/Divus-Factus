#!/bin/bash
# Double-click launcher for the Atelier — the maker's own bench.
#
# Finder opens .command files in a fresh Terminal rooted at the home directory and
# with a login shell that may not have picked up the Rust toolchain, so this both
# moves to the Atelier and puts cargo on PATH before doing anything.

cd "$(dirname "$0")/atelier" || exit 1

# rustup installs here; harmless if it is already on PATH.
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. Install Rust from https://rustup.rs and try again."
    echo
    read -r -p "Press Return to close."
    exit 1
fi

echo "Building the Atelier..."
echo

# The Atelier is its own program with its own build; the game's assets and
# palette reach it as files, not code. Release for the same reason the game
# launches release: it is tuned against real performance.
cargo run --release

status=$?
echo

if [ $status -ne 0 ]; then
    # Keep the window open so build errors are readable instead of vanishing.
    echo "The Atelier exited with status $status."
    read -r -p "Press Return to close."
fi
