#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
BRANCH=${BRANCH:-bevy}
OUT=dist scripts/web.sh
wrangler pages deploy dist --project-name kids-free-games --branch "$BRANCH" --commit-dirty=true
