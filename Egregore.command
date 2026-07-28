#!/bin/bash
# Double-click launcher for Egregore.
#
# Finder opens .command files in a fresh Terminal rooted at the home directory and
# with a login shell that may not have picked up the Rust toolchain, so this both
# moves to the project and puts cargo on PATH before doing anything.

cd "$(dirname "$0")" || exit 1

# rustup installs here; harmless if it is already on PATH.
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not found. Install Rust from https://rustup.rs and try again."
    echo
    read -r -p "Press Return to close."
    exit 1
fi

echo "Building Egregore..."
echo

# Assets resolve relative to the manifest, so the game must be launched through
# cargo rather than by running the binary directly. Release, always: the game
# is tuned and tested against release performance, and debug frame rates would
# misrepresent it.
cargo run --release

status=$?
echo

if [ $status -ne 0 ]; then
    # Keep the window open so build errors are readable instead of vanishing.
    echo "Egregore exited with status $status."
    read -r -p "Press Return to close."
fi
