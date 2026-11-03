#!/bin/sh
# Explicit, source-independent runtime qualification of an existing package.
# Run with Backplane's bend lock on a host meeting the test admission floor.
# No build, install, live target, private input or forced GC is supported.
set -eu
cd "$(dirname "$0")/../.."
[ "$#" = 2 ] || { echo "usage: $0 PACKAGED_HUB EXPECTED_SHA256" >&2; exit 2; }
. scripts/heavy-guard.sh
heavy_guard test "$@"
python3 - "$1" "$2" <<'VERIFY'
import hashlib,sys
from pathlib import Path
binary=Path(sys.argv[1]).resolve(strict=True)
expected=sys.argv[2]
if len(expected)!=64 or any(c not in '0123456789abcdef' for c in expected):
    raise SystemExit('expected SHA256 must be 64 lowercase hex characters')
h=hashlib.sha256()
with binary.open('rb') as f:
    for chunk in iter(lambda:f.read(1048576),b''): h.update(chunk)
if h.hexdigest()!=expected: raise SystemExit('packaged hub SHA256 mismatch')
VERIFY
exec bun test/tools/hub_memory_soak.ts --bin "$1" --out build/ahead-validation/memory/report.json
