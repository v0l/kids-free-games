#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
attach=
for arg in "$@"; do
  case $arg in
    --attach) attach=1 ;;
    *) echo "usage: $0 [--attach]" >&2; exit 2 ;;
  esac
done
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
IFS=. read -r major minor patch <<< "${version%%[-+]*}"
build=${BUILD:-$(( major * 10000 + minor * 100 + patch ))}

if ! ssh -o ConnectTimeout=8 -o BatchMode=yes mac true 2>/dev/null; then
  cat >&2 <<'EOF'
The Mac is not reachable over ssh. If it rebooted, FileVault is waiting at the pre-boot screen:
  1. From a machine on the LAN run: ssh -o ControlPath=none kieran@<mac ip>   (the IP is DHCP, it was 10.100.2.202 and 10.100.2.203)
  2. Type the Mac login password. The session drops and macOS finishes booting.
  3. Wait a few minutes, check with: ssh -o ConnectTimeout=8 mac true
See ~/.agents/memory/mac-dev-server.md.
EOF
  exit 1
fi

rsync -az --delete --exclude target --exclude .git --exclude assets/music --exclude docs/shots --exclude dist ./ mac:git/kids-free-games/
remote='export PATH=$HOME/.cargo/bin:/opt/homebrew/bin:$PATH; cd ~/git/kids-free-games'
ssh mac "$remote && BUILD=$build scripts/ios-appstore.sh"
ssh mac "$remote && BUILD=$build scripts/macos-pkg.sh"

mkdir -p dist
ipa=dist/kids_games-$tag-ios.ipa
pkg=dist/kids_games-$tag-macos.pkg
scp -q mac:git/kids-free-games/target/ios/KidsGames.ipa "$ipa"
scp -q mac:git/kids-free-games/target/macos/KidsGames.pkg "$pkg"

ipa_version=$(unzip -p "$ipa" 'Payload/KidsGames.app/Info.plist' | python3 -c 'import plistlib,sys; p=plistlib.loads(sys.stdin.buffer.read()); print(p["CFBundleShortVersionString"], p["CFBundleVersion"])')
read -r ipa_short ipa_build <<< "$ipa_version"
if [ "$ipa_short" != "$version" ]; then
  echo "IPA CFBundleShortVersionString is $ipa_short, Cargo version is $version" >&2
  exit 1
fi
if [ "$ipa_build" != "$build" ]; then
  echo "IPA CFBundleVersion is $ipa_build, expected build $build" >&2
  exit 1
fi

(cd dist && sha256sum "$(basename "$ipa")" "$(basename "$pkg")" | tee "kids_games-$tag-apple.sha256")
if [ -n "$attach" ]; then
  gh release upload "$tag" "$ipa" "$pkg" "dist/kids_games-$tag-apple.sha256" --clobber --repo v0l/kids-free-games
fi
echo "$ipa"
echo "$pkg"
