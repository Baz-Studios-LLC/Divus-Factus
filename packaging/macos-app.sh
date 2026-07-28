#!/usr/bin/env bash
# Assemble Egregore.app around an already-built release binary.
# Usage: packaging/macos-app.sh <path-to-binary> <version> [out-dir]
# Unlike WriftHeart, Egregore loads real files from assets/ (shaders, the
# title logo), so the bundle carries the assets folder BESIDE the binary —
# Bevy resolves the asset root from the executable's own directory.
set -euo pipefail
BIN="${1:?usage: macos-app.sh <binary> <version> [out-dir]}"
VERSION="${2:?need a version}"
OUT="${3:-dist}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

APP="$OUT/Egregore.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/egregore"
chmod +x "$APP/Contents/MacOS/egregore"
strip "$APP/Contents/MacOS/egregore" 2>/dev/null || true

cp -R "$ROOT/assets" "$APP/Contents/MacOS/assets"
sed "s/__VERSION__/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# Ad-hoc sign so macOS runs it without a "damaged" error; the launcher also
# strips the download quarantine on install.
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo "built $APP ($(du -sh "$APP" | cut -f1))"
