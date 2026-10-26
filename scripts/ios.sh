#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=${TARGET:-aarch64-apple-ios-sim}
BUNDLE_ID=${BUNDLE_ID:-io.v0l.kidsgames}
PROFILE=${PROFILE:-small}
APP=target/ios/KidsGames.app
start=$(date +%s)
cargo build -p kids_games --bin kids_games --profile "$PROFILE" --target "$TARGET"
rm -rf "$APP"
mkdir -p "$APP"
cp "target/$TARGET/$PROFILE/kids_games" "$APP/KidsGames"
sips -z 120 120 ios/icon-1024.png --out "$APP/AppIcon60x60@2x.png" >/dev/null
sips -z 152 152 ios/icon-1024.png --out "$APP/AppIcon76x76@2x~ipad.png" >/dev/null
sips -z 167 167 ios/icon-1024.png --out "$APP/AppIcon83.5x83.5@2x~ipad.png" >/dev/null
sed "s/__BUNDLE_ID__/$BUNDLE_ID/" ios/Info.plist > "$APP/Info.plist"
echo "APPL????" > "$APP/PkgInfo"
if [ "$TARGET" = aarch64-apple-ios ]; then
  codesign --force --sign "${SIGN_IDENTITY:?set SIGN_IDENTITY for a device build}" \
    ${PROFILE_PLIST:+--entitlements "$PROFILE_PLIST"} "$APP"
else
  codesign --force --sign - "$APP"
fi
echo "binary: $(( $(stat -f %z "$APP/KidsGames") / 1024 )) KiB, built in $(( $(date +%s) - start )) s"
echo "$APP"
