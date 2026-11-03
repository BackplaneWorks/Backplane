#!/bin/sh
# For a heavy command without its own guarded script. Pass the workload class
# followed by -- and the actual command, so the guard owns its entire lifetime.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec python3 "$root/scripts/heavy-guard.py" "$@"
