#!/usr/bin/env bash
# Extract the launcher icons from the player's Plants vs. Zombies install.
#
# The icons are PopCap game art, so they are not committed. This regenerates
# the three files used by the README and the macOS packaging script:
#
#   github/pvz.icns       copied from the game's Lawn.icns
#   github/pvz-icon.png   512px PNG from the same icon
#   github/pvz.ico        256px Windows icon
#
# Usage: scripts/extract-icons.sh [output-dir]   (default: <repo>/github)
# Set PVZ_GAME_PATH to the install folder, the .app, or main.pak.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/github}"

die() {
  echo "extract-icons: $*" >&2
  exit 1
}

command -v iconutil >/dev/null 2>&1 || die "macOS iconutil is required"
command -v sips >/dev/null 2>&1 || die "macOS sips is required"

find_icon() {
  local base found
  for base in \
    "${PVZ_GAME_PATH:-}" \
    "$HOME/Library/Application Support/Steam/steamapps/common/Plants Vs Zombies" \
    "$HOME/Library/Application Support/Steam/steamapps/common/Plants vs. Zombies" \
    "$HOME/.steam/steam/steamapps/common/Plants vs. Zombies" \
    "$HOME/.local/share/Steam/steamapps/common/Plants vs. Zombies"; do
    [ -n "$base" ] || continue
    [ -e "$base" ] || continue
    if [ -f "$base" ]; then
      base="$(dirname "$base")"
    fi

    found="$(find "$base" -maxdepth 6 -type f -iname 'Lawn.icns' -print -quit 2>/dev/null || true)"
    if [ -n "$found" ]; then
      printf '%s\n' "$found"
      return 0
    fi
    found="$(find "$base" -maxdepth 6 -type f -iname '*.icns' -print -quit 2>/dev/null || true)"
    if [ -n "$found" ]; then
      printf '%s\n' "$found"
      return 0
    fi
  done
  return 1
}

ICNS="$(find_icon || true)"
if [ -z "$ICNS" ]; then
  die "no game icon found; set PVZ_GAME_PATH to the game install, .app, or main.pak"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

iconutil -c iconset -o "$tmp/Lawn.iconset" "$ICNS"
png512="$tmp/Lawn.iconset/icon_512x512.png"
png256="$tmp/Lawn.iconset/icon_256x256.png"
if [ ! -f "$png512" ]; then
  png512="$(find "$tmp/Lawn.iconset" -type f -name 'icon_512x512*.png' -print -quit)"
fi
if [ ! -f "$png256" ]; then
  png256="$(find "$tmp/Lawn.iconset" -type f -name 'icon_256x256*.png' -print -quit)"
fi
[ -n "$png512" ] && [ -f "$png512" ] || die "the game icon has no 512px frame"
[ -n "$png256" ] && [ -f "$png256" ] || die "the game icon has no 256px frame"

mkdir -p "$OUT"
cp "$ICNS" "$OUT/pvz.icns"
cp "$png512" "$OUT/pvz-icon.png"
sips -s format ico "$png256" --out "$OUT/pvz.ico" >/dev/null

echo "Extracted icons from $ICNS to $OUT"
