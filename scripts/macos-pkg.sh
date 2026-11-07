#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
DIST=${DIST:-$HOME/kgdist}
KC=$DIST/kg.keychain-db
PW=kgbuild
TARGET=aarch64-apple-darwin
PROFILE=${PROFILE:-small}
security delete-keychain "$KC" 2>/dev/null || true
security create-keychain -p "$PW" "$KC"
security set-keychain-settings -t 3600 "$KC"
security unlock-keychain -p "$PW" "$KC"
security import "$DIST/dist.p12" -k "$KC" -P kg -T /usr/bin/codesign -T /usr/bin/productbuild >/dev/null
security import "$DIST/inst.p12" -k "$KC" -P kg -T /usr/bin/productbuild -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign:,productbuild: -s -k "$PW" "$KC" >/dev/null
security list-keychains -d user -s "$KC" $(security list-keychains -d user | tr -d '"')
APP_IDENTITY=$(security find-identity -v -p codesigning "$KC" | grep -o 'Apple Distribution[^"]*' | head -1)
PKG_IDENTITY=$(security find-identity -v "$KC" | grep -o '3rd Party Mac Developer Installer[^"]*' | head -1)
: "${APP_IDENTITY:?no Apple Distribution identity in $KC}"
: "${PKG_IDENTITY:?no 3rd Party Mac Developer Installer identity in $KC}"
VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
BUILD=${BUILD:-$VERSION}
cargo build -p kids_games --bin kids_games --profile "$PROFILE" --target "$TARGET"
BIN="target/$TARGET/$PROFILE/kids_games" VERSION=$VERSION BUILD=$BUILD OUT=target/macos \
  SIGN_IDENTITY="$APP_IDENTITY" KEYCHAIN="$KC" ENTITLEMENTS=packaging/macos/sandbox.entitlements scripts/macos-app.sh
rm -f target/macos/KidsGames.pkg
productbuild --component target/macos/KidsGames.app /Applications --sign "$PKG_IDENTITY" --keychain "$KC" target/macos/KidsGames.pkg
echo "target/macos/KidsGames.pkg"
