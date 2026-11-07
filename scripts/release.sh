#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
tag=${TAG:-v$version}
if [ "$tag" != "v$version" ]; then
  echo "tag $tag does not match the workspace version $version" >&2
  exit 1
fi
notes=$(awk -v v="$version" '/^## \[/{p=index($0,"[" v "]")>0;next} /^\[[^]]*\]: /{p=0} p' CHANGELOG.md)
if ! grep -q '[^[:space:]]' <<< "$notes"; then
  echo "CHANGELOG.md has no entry for $version" >&2
  exit 1
fi
if [ -n "${KEYSTORE_BASE64:-}" ]; then
  keystore=$(mktemp)
  trap 'rm -f "$keystore"' EXIT
  base64 -d <<< "$KEYSTORE_BASE64" > "$keystore"
  export KG_KEYSTORE=$keystore
elif [ -n "${KEYSTORE:-}" ]; then
  export KG_KEYSTORE=$KEYSTORE
fi
if [ -n "${KG_KEYSTORE:-}" ]; then
  export KG_KEYSTORE_PASSWORD=${KEYSTORE_PASS:?KEYSTORE_PASS is needed with a keystore}
  export KG_KEY_ALIAS=${KEY_ALIAS:?KEY_ALIAS is needed with a keystore}
  export KG_KEY_PASSWORD=${KEY_PASS:-$KEYSTORE_PASS}
  suffix=android-arm64
else
  echo "no keystore given: signing with the debug key" >&2
  suffix=debug-signed
fi
ABI=${ABI:-arm64-v8a} scripts/android.sh
mkdir -p dist
apk="dist/kids_games-$version-$suffix.apk"
cp target/android-artifacts/kids_games.apk "$apk"
(cd dist && sha256sum "$(basename "$apk")" | tee "$(basename "$apk").sha256")
echo "$apk"
