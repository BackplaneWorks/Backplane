#!/bin/sh
# desktop side: upload only what the markdown oracle needs and run it on Boat. usage from the repo root: N=30000 SEED=2 sh test/screen/md_oracle_launch.sh
set -eu
INC=$( { python3 - <<'PY'
import re,os
seen=set();todo=['test/screen/md_dump']
while todo:
    f=todo.pop()
    if f in seen: continue
    seen.add(f)
    for m in re.finditer(r'^import',open(f).read(),re.M):
        p=os.path.normpath(os.path.join(os.path.dirname(f),m.group(1)))
        if os.path.exists(p): todo.append(p)
print('\n'.join(sorted(seen)))
PY
git ls-files -co --exclude-standard native/host test/screen test/agents/fixtures test/files/hostile_paths.tsv | grep -v '^native/host/target'
echo src/mobile/screen; echo src/core/cbor; } | sed 's/^/--include-file /' | tr '\n' ' ')
# shellcheck disable=SC2086
exec scripts/boat-job.sh --size large --timeout 1800 --idle 3600 --artifact out/md $INC -- env N="${N:-3000}" SEED="${SEED:-1}" sh test/screen/md_oracle.sh
