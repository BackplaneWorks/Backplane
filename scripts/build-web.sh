#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/web"

if ! command -v bun >/dev/null 2>&1; then
  echo "build-web: bun is required to rebuild web/dist (cargo embeds the committed web/dist without bun)" >&2
  exit 1
fi

exec bun build.ts "$@"
