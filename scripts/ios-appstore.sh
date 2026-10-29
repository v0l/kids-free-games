#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
DIST=${DIST:-$HOME/kgdist}
KC=$HOME/kgdist/kg.keychain-db
PW=kgbuild
security delete-keychain "$KC" 2>/dev/null || true
security create-keychain -p "$PW" "$KC"
security set-keychain-settings -t 3600 "$KC"
security unlock-keychain -p "$PW" "$KC"
security import "$DIST/dist.p12" -k "$KC" -P kg -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$PW" "$KC" >/dev/null
security list-keychains -d user -s "$KC" $(security list-keychains -d user | tr -d '"')
IDENTITY=$(security find-identity -v -p codesigning "$KC" | grep -o 'Apple Distribution[^"]*' | head -1)
BUILD=${BUILD:-1}
sed -i '' "/<key>CFBundleVersion<\/key>/{n;s|<string>.*</string>|<string>$BUILD</string>|;}" ios/Info.plist
TARGET=aarch64-apple-ios PROFILE=${PROFILE:-small} KEYCHAIN="$KC" SIGN_IDENTITY="$IDENTITY" PROVISION="$DIST/kg.mobileprovision" scripts/ios.sh
rm -rf target/ios/ipa target/ios/KidsGames.ipa
mkdir -p target/ios/ipa/Payload
cp -R target/ios/KidsGames.app target/ios/ipa/Payload/
(cd target/ios/ipa && /usr/bin/zip -qry ../KidsGames.ipa Payload)
echo "target/ios/KidsGames.ipa"
