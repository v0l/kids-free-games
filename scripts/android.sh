#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
docker build -q -t kids-games-apk android >/dev/null
mkdir -p target/android-artifacts
docker run --rm \
  -v "$PWD":/repo:ro \
  -v "$PWD/target/android-artifacts":/out \
  -v kids-games-cargo:/usr/local/cargo/registry \
  -v kids-games-apk-target:/root/src/target \
  -v kids-games-android-key:/root/.android \
  kids-games-apk sh -euc "
    cd /root/src
    tar -C /repo --exclude=./target --exclude=./.git -cf - . | tar -xf -
    sed -i 's/^version = 4$/version = 3/' Cargo.lock
    cargo quad-apk build --release
    cp target/android-artifacts/release/apk/*.apk /out/
    chown -R $(id -u):$(id -g) /out
  "
ls -la target/android-artifacts/*.apk
