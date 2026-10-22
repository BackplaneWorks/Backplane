#!/usr/bin/env bash
set -euo pipefail
export BEND_NO_TELEMETRY=1 CHECK_JOBS=1 CHECK_SHARDS=1
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912
out=build/mailrun-dispositions
mkdir -p "$out"
source scripts/bendok.sh
python3 - "$1" <<'PY'
import hashlib,json,sys
from pathlib import Path
paths=['src/core/maildisposition.bend','src/core/mcp.bend','src/core/mailrun.bend','src/core/mailcontinuity.bend','src/core/hub.bend','src/server/server.bend','LAWS.bend','PROOF.bend','test/maildisposition_test.bend','test/tools/maildisposition_gate.sh','test/fixtures/maildisposition-before.patch','docs/harness/mail-continuity.md']
pins={p:hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in paths}
key=hashlib.sha256(json.dumps(pins,sort_keys=True,separators=(',',':')).encode()).hexdigest()
assert key==sys.argv[1],(key,sys.argv[1])
Path('build/mailrun-dispositions/inputs.json').write_text(json.dumps({'key':key,'paths':pins},indent=2)+'\n')
PY
for f in src/core/maildisposition.bend test/maildisposition_test.bend src/server/server.bend; do
  bend_check "$f" > "$out/$(basename "$f").check" 2>&1 || { tail -65 "$out/$(basename "$f").check"; exit 1; }
  echo "$f narrow PASS"
done
# Standalone proofs of only the six additive statements; old LAWS/PROOF
# remain byte-preserved and the whole proof runs after pure regressions.
python3 - <<'PY'
from pathlib import Path
l=Path('LAWS.bend').read_text().split('# Permission for notice bookkeeping is not permission for underlying work.\n')[1]
p=Path('PROOF.bend').read_text().split('def Laws.mail_disposition_inactive_refused',1)[1]
Path('build/mailrun-dispositions/new-laws.bend').write_text('import Base\nimport ../../src/core/maildisposition.bend as MDSP\n'+l+'\ndef mail_disposition_inactive_refused'+p.replace('def Laws.','def '))
PY
rc=0
bend "$out/new-laws.bend" > "$out/new-laws.log" 2>&1 || rc=$?
bend_verdict "$rc" "$(cat "$out/new-laws.log")" || { tail -50 "$out/new-laws.log"; exit 1; }
echo 'Six new production laws PASS'
# Reproduce the exact routine-hold failure with the old shipped MCP and
# continuity path. Restore patched files even if the red check fails.
python3 - <<'PY'
from pathlib import Path
import subprocess
out=Path('build/mailrun-dispositions')
paths=['src/core/mcp.bend','src/core/hub.bend','src/core/mailrun.bend','src/core/mailcontinuity.bend']
for i,p in enumerate(paths):
    (out/f'patched-{i}').write_bytes(Path(p).read_bytes())
subprocess.run(['git','apply','--unidiff-zero','--reverse','test/fixtures/maildisposition-before.patch'],check=True)
source=Path('test/maildisposition_test.bend').read_text().split('def main() -> IO(Unit):')[0]
source=source.replace('import ./check.bend','import ../../test/check.bend').replace('import ../src/','import ../../src/')
source+='''def main() -> IO(Unit):
  do IO<Unit>:
    b("shipped incident cannot record bounded notice", MC.Res.good(call(base(), args(input(), "done"))), True{})
    n("shipped done cannot stop repeated known hold escalation", owes(apply(base(), call(base(), args(input(), "done")))), 0n)
    n("shipped defer cannot durably retain the existing hold", owes(apply(base(), call(base(), args(input(), "defer")))), 0n)
'''
(out/'before.bend').write_text(source)
PY
restore() {
  local i=0
  for f in src/core/mcp.bend src/core/hub.bend src/core/mailrun.bend src/core/mailcontinuity.bend; do
    cp "$out/patched-$i" "$f"; i=$((i+1))
  done
}
trap restore EXIT
bend "$out/before.bend" > "$out/before.log" 2>&1 || { tail -50 "$out/before.log"; exit 1; }
test "$(grep -c '^FAIL ' "$out/before.log")" = 3 || { cat "$out/before.log"; exit 1; }
restore
trap - EXIT
echo 'Three actual shipped routine-hold regressions reproduced'
for f in maildisposition mailcontinuity mailrun mailrun_model; do
  bend "test/${f}_test.bend" > "$out/$f.log" 2>&1 || { tail -60 "$out/$f.log"; exit 1; }
  grep -q '^ok ' "$out/$f.log"
  if grep -Eq '^(FAIL|Error:)' "$out/$f.log"; then grep -E '^(FAIL|Error:)' "$out/$f.log"; exit 1; fi
  echo "$f PASS ($(grep -c '^ok ' "$out/$f.log") assertions)"
done
scripts/check.sh all > "$out/check-proof.log" 2>&1 || { tail -65 "$out/check-proof.log"; exit 1; }
python3 - <<'PY'
import datetime,hashlib,json
from pathlib import Path
out=Path('build/mailrun-dispositions'); pins=json.loads((out/'inputs.json').read_text())
for p,h in pins['paths'].items(): assert hashlib.sha256(Path(p).read_bytes()).hexdigest()==h,p
logs={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob('*.log')}
counts={n:sum(l.startswith('ok ') for l in (out/(n+'.log')).read_text().splitlines()) for n in ['maildisposition','mailcontinuity','mailrun','mailrun_model']}
result={'status':'PASS','at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source_key':pins['key'],'assertions':counts,'logs':logs,'scope':'source/full proof and pure production-path tests; actual CLI approval/native integration NOT qualified'}
(out/'validation.json').write_text(json.dumps(result,indent=2)+'\n'); print(json.dumps(result,indent=2))
PY
