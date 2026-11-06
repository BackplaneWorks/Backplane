#!/bin/sh
# desktop side: upload only what the markdown oracle needs and run it on Boat. usage from the repo root: N=30000 SEED=2 sh test/gpui/screen/md_oracle_launch.sh
set -eu
INC=$( { python3 - <<'PY'
import re,os
seen=set();todo=['test/gpui/screen/md_dump.bend']
while todo:
    f=todo.pop()
    if f in seen: continue
    seen.add(f)
    for m in re.finditer(r'^import (\S+\.bend)',open(f).read(),re.M):
        p=os.path.normpath(os.path.join(os.path.dirname(f),m.group(1)))
        if os.path.exists(p): todo.append(p)
print('\n'.join(sorted(seen)))
PY
git ls-files -co --exclude-standard native/host test/gpui/screen test/gpui/agents/fixtures test/gpui/files/hostile_paths.tsv | grep -v '^native/host/target'
echo src/mobile/screen.bend; echo src/core/cbor.bend; } | sed 's/^/--include-file /' | tr '\n' ' ')
# shellcheck disable=SC2086
exec scripts/boat-job.sh --size large --timeout 1800 --idle 3600 --artifact out/md $INC -- env N="${N:-3000}" SEED="${SEED:-1}" sh test/gpui/screen/md_oracle.sh
