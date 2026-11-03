#!/bin/sh
# Run only on an owner-controlled private SMALL VM. Inputs are staged outside
# git by its owner; this script never downloads a store, builds or installs.
set -eu
: "${BACKPLANE_QUALIFIED_HUB:?private runner needs an immutable packaged service copy}"
: "${BACKPLANE_QUALIFIED_HUB_SHA256:?private runner needs its qualified SHA256}"
: "${BACKPLANE_PRIVATE_REAL_STORE:?private runner needs the owner-supplied fixture archive}"
python3 - <<'PY'
from pathlib import Path
m={r.split(':')[0]:int(r.split()[1])*1024 for r in Path('/proc/meminfo').read_text().splitlines() if r.startswith(('MemTotal:','MemAvailable:'))}
assert 0<m['MemTotal']<4*1024**3, 'private real-store gate requires normal SMALL VM, below4GiB'
assert m['MemAvailable']>1024**3, 'private real-store gate reserve refused'
PY
exec 9>/tmp/bp-bend.lock
flock -n 9
umask 077
mkdir -p build/private-store-gate
exec bun test/tools/real_store_recovery.ts --bin "$BACKPLANE_QUALIFIED_HUB" --sha256 "$BACKPLANE_QUALIFIED_HUB_SHA256" --fixture "$BACKPLANE_PRIVATE_REAL_STORE" --out build/private-store-gate/result.json --seconds 1800
