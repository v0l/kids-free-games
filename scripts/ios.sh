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
mkdir -p "$APP/assets/art"
cp assets/art/*.kga "$APP/assets/art/"
mkdir -p "$APP/assets/models"
cp assets/models/*.glb "$APP/assets/models/"
cp assets/sound.kgs assets/sound_words.kgs "$APP/assets/"
sips -z 120 120 ios/icon-1024.png --out "$APP/AppIcon60x60@2x.png" >/dev/null
sips -z 152 152 ios/icon-1024.png --out "$APP/AppIcon76x76@2x~ipad.png" >/dev/null
sips -z 167 167 ios/icon-1024.png --out "$APP/AppIcon83.5x83.5@2x~ipad.png" >/dev/null
sed "s/__BUNDLE_ID__/$BUNDLE_ID/" ios/Info.plist > "$APP/Info.plist"
if [ "$TARGET" = aarch64-apple-ios ]; then
  icons=target/ios/Assets.xcassets/AppIcon.appiconset
  rm -rf target/ios/Assets.xcassets
  mkdir -p "$icons"
  cp ios/icon-1024.png "$icons/icon-1024.png"
  echo '{"images":[{"filename":"icon-1024.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"xcode","version":1}}' > "$icons/Contents.json"
  echo '{"info":{"author":"xcode","version":1}}' > target/ios/Assets.xcassets/Contents.json
  xcrun actool target/ios/Assets.xcassets --compile "$APP" --platform iphoneos --minimum-deployment-target 13.0 \
    --app-icon AppIcon --output-partial-info-plist target/ios/icons.plist --target-device iphone --target-device ipad >/dev/null
  /usr/libexec/PlistBuddy -c "Merge target/ios/icons.plist" "$APP/Info.plist"
fi
echo "APPL????" > "$APP/PkgInfo"
if [ "$TARGET" = aarch64-apple-ios ]; then
  if [ -n "${PROVISION:-}" ]; then
    cp "$PROVISION" "$APP/embedded.mobileprovision"
    security cms -D -i "$PROVISION" > target/ios/profile.plist
    plutil -extract Entitlements xml1 -o target/ios/entitlements.plist target/ios/profile.plist
    team=$(plutil -extract TeamIdentifier.0 raw -o - target/ios/profile.plist)
    plutil -replace application-identifier -string "$team.$BUNDLE_ID" target/ios/entitlements.plist
    PROFILE_PLIST=target/ios/entitlements.plist
  fi
  codesign --force --sign "${SIGN_IDENTITY:?set SIGN_IDENTITY for a device build}" \
    ${KEYCHAIN:+--keychain "$KEYCHAIN"} ${PROFILE_PLIST:+--entitlements "$PROFILE_PLIST"} "$APP"
else
  codesign --force --sign - "$APP"
fi
echo "binary: $(( $(stat -f %z "$APP/KidsGames") / 1024 )) KiB, built in $(( $(date +%s) - start )) s"
echo "$APP"
