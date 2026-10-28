#!/usr/bin/env bash
# Build the release pvz and wrap it in a .app with the Plants vs. Zombies icon.
# The icon is extracted from the player's game install; set PVZ_GAME_PATH if
# the game is not in the default Steam location.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [ ! -f "$ROOT/github/pvz.icns" ]; then
  "$ROOT/scripts/extract-icons.sh"
fi
cargo build -p pvz --release
BIN="$ROOT/target/release/pvz"
APP="$ROOT/target/release/Plants vs. Zombies.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/pvz"
cp "$ROOT/github/pvz.icns" "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Plants vs. Zombies</string>
  <key>CFBundleDisplayName</key><string>Plants vs. Zombies</string>
  <key>CFBundleIdentifier</key><string>io.github.pvz-goty-rs</string>
  <key>CFBundleExecutable</key><string>pvz</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "Built $APP"
