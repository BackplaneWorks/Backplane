#!/bin/sh
# Shared machine memory is serialized across proof, emit and native test lanes.
set -eu
cd "$(dirname "$0")/.."
if [ "${BACKPLANE_VALIDATION_LOCKED:-}" != 1 ] && command -v flock >/dev/null 2>&1; then
  export BACKPLANE_VALIDATION_LOCKED=1
  exec flock /tmp/bp-wt-build.lock "$0" "$@"
fi
exec python3 scripts/validate-contribution.py "$@"
