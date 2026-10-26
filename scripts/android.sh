#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}
export JAVA_HOME=${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk-amd64}
PROFILE=${PROFILE:-small}
ABI=${ABI:-arm64-v8a}
start=$(date +%s)
echo "sdk.dir=$ANDROID_HOME" > android/local.properties
rm -rf android/app/src/main/jniLibs android/app/src/main/assets
mkdir -p android/app/src/main/assets/art
cp assets/art/*.kga android/app/src/main/assets/art/
cp assets/sound.kgs assets/sound_words.kgs android/app/src/main/assets/
cargo ndk -t "$ABI" -P 26 -o android/app/src/main/jniLibs \
  build -p kids_games --lib --profile "$PROFILE"
so=android/app/src/main/jniLibs/$ABI/libkids_games.so
"$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip "$so"
(cd android && ./gradlew -q assembleRelease -Pabi="$ABI")
mkdir -p target/android-artifacts
cp android/app/build/outputs/apk/release/app-release.apk target/android-artifacts/kids_games.apk
echo "so: $(( $(stat -c %s "$so") / 1024 )) KiB, apk: $(( $(stat -c %s target/android-artifacts/kids_games.apk) / 1024 )) KiB, built in $(( $(date +%s) - start )) s"
echo target/android-artifacts/kids_games.apk
