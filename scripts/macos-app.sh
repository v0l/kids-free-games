#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=${BIN:?BIN is the path to the built kids_games binary}
OUT=${OUT:-target/macos}
VERSION=${VERSION:-$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
BUILD=${BUILD:-$VERSION}
SIGN_IDENTITY=${SIGN_IDENTITY:--}
APP=$OUT/KidsGames.app
rm -rf "$APP" "$OUT/icon.iconset"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/assets/art" "$APP/Contents/Resources/assets/models" "$OUT/icon.iconset"
sed -e "s/__VERSION__/$VERSION/g" -e "s/__BUILD__/$BUILD/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" ios/icon-1024.png --out "$OUT/icon.iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) ios/icon-1024.png --out "$OUT/icon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$OUT/icon.iconset" -o "$APP/Contents/Resources/icon.icns"
rm -rf "$OUT/icon.iconset"
cp "$BIN" "$APP/Contents/MacOS/kids_games"
cp assets/art/*.kga "$APP/Contents/Resources/assets/art/"
cp assets/models/* "$APP/Contents/Resources/assets/models/"
cp assets/sound.kgs assets/sound_words.kgs assets/Fredoka-SemiBold.ttf "$APP/Contents/Resources/assets/"
echo "APPL????" > "$APP/Contents/PkgInfo"
if [ -n "${ENTITLEMENTS:-}" ]; then
  codesign --force --timestamp --options runtime --sign "$SIGN_IDENTITY" ${KEYCHAIN:+--keychain "$KEYCHAIN"} --entitlements "$ENTITLEMENTS" "$APP"
else
  codesign --force --sign "$SIGN_IDENTITY" ${KEYCHAIN:+--keychain "$KEYCHAIN"} "$APP"
fi
echo "$APP"
