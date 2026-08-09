#!/bin/bash
# Double-click launcher for Divus Factus.
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

echo "Building Divus Factus..."
echo

# Assets resolve relative to the manifest, so the game must be launched through
# cargo rather than by running the binary directly. Release, always: the game
# is tuned and tested against release performance, and debug frame rates would
# misrepresent it.
#
# Every session leaves a readable trail: the same output that scrolls here is
# teed into logs/latest.log (the previous session kept beside it), so a death
# or a stall can be read after the fact — or watched live with
#   tail -f logs/latest.log
mkdir -p logs
[ -f logs/latest.log ] && mv -f logs/latest.log logs/previous.log
cargo run --release 2>&1 | tee logs/latest.log

status=${PIPESTATUS[0]}
echo

if [ $status -ne 0 ]; then
    # Keep the window open so build errors are readable instead of vanishing.
    echo "Divus Factus exited with status $status."
    read -r -p "Press Return to close."
fi
