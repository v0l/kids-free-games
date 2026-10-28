#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
bun install --frozen-lockfile
bun test
bun run build
cp dist/index.html ../src/admin.html
