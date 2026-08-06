#!/usr/bin/env bash
# Assemble DivusFactus.app around an already-built release binary.
# Usage: packaging/macos-app.sh <path-to-binary> <version> [out-dir]
# Unlike WriftHeart, Divus Factus loads real files from assets/ (shaders, the
# title logo), so the bundle carries the assets folder BESIDE the binary —
# Bevy resolves the asset root from the executable's own directory.
#
# The bundle is named without a space on purpose: Finder shows the spaced name
# from CFBundleDisplayName anyway, and a spaced path is one more thing for tar,
# codesign and the launcher to get right. The launcher finds the bundle by its
# `.app` extension rather than by name, so this can be called anything.
set -euo pipefail
BIN="${1:?usage: macos-app.sh <binary> <version> [out-dir]}"
VERSION="${2:?need a version}"
OUT="${3:-dist}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

APP="$OUT/DivusFactus.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# The executable's name must match CFBundleExecutable in Info.plist, or macOS
# refuses to launch the bundle.
cp "$BIN" "$APP/Contents/MacOS/divus-factus"
chmod +x "$APP/Contents/MacOS/divus-factus"
strip "$APP/Contents/MacOS/divus-factus" 2>/dev/null || true

cp -R "$ROOT/assets" "$APP/Contents/MacOS/assets"

# The Atelier rides along, if it has been built. The title screen opens it and
# stands down, so the bench a player reaches is always the one that matches the
# game it feeds - the two share a file contract, and a bench a release behind
# writes buildings the game reads differently.
#
# Its own crate, so its own target directory; the workflow builds it before
# calling this. Missing is not an error: the game hides the button when there is
# no bench beside it, and a build without one is simply a game.
BENCH="$ROOT/atelier/target/release/divus-factus-atelier"
if [ -f "$BENCH" ]; then
  # The same name it takes on Windows. A bundle names its own executable in
  # Info.plist so nothing here was ever at risk of being launched by mistake -
  # but one name for the bench on both platforms is one name to look for.
  cp "$BENCH" "$APP/Contents/MacOS/TheAtelier"
  chmod +x "$APP/Contents/MacOS/TheAtelier"
  strip "$APP/Contents/MacOS/TheAtelier" 2>/dev/null || true
  # The palette the bench paints from, exported by the game. Its fonts come out
  # of the game's own assets folder, which is already beside them.
  cp -R "$ROOT/atelier/data" "$APP/Contents/MacOS/data"
  echo "  with the Atelier"
fi
# Likewise the icon: CFBundleIconFile names it without the extension.
cp "$HERE/DivusFactus.icns" "$APP/Contents/Resources/DivusFactus.icns"
sed "s/__VERSION__/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# Ad-hoc sign so macOS runs it without a "damaged" error; the launcher also
# strips the download quarantine on install.
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo "built $APP ($(du -sh "$APP" | cut -f1))"
