#!/bin/bash
# embed_perf_e2e.sh (cold Edit, tab switches, save) on copies of the real boards, each runtime, one after another; a table at the end.
#   REAL_HARDWARE=1 test/kicad_embed/real_use_baseline.sh [OUT_ROOT]      (KEMBED_BOARDS="hubble" KEMBED_RUNTIMES="bp12" to narrow)
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$HERE/../.." && pwd)
. "$REPO_ROOT/scripts/lib-bptmp.sh"
. "$HERE/lib/real_use.sh"
ROOT=${1:-$(mktemp -d "$BP_TMP/bp-realuse.XXXXXX")}
mkdir -p "$ROOT"
one() {
  local bn=$1 bs=$2 rn=$3 rk=$4 run="$ROOT/$1-$3"
  stage_board "$bs" "$ROOT/src-$bn"
  echo "=== $bn on $rn ($rk) -> $run"
  OUT="$run" EMBED_MODE=fork BACKPLANE_KICAD_BIN="$rk" BACKPLANE_KICAD_CLI="$(dirname "$rk")/kicad-cli" KEMBED_PROJ_SRC="$ROOT/src-$bn" \
    KEMBED_PERF_STRICT=0 "$HERE/embed_perf_e2e.sh" >"$ROOT/$bn-$rn.log" 2>&1
  echo "rc=$?" >>"$ROOT/$bn-$rn.log"
  grep -h "^PERF\|^PASS\|^FAIL\|rc=" "$ROOT/$bn-$rn.log"
  grep -h '"event":"gpu"' "$run/client.log" 2>/dev/null | head -1 | sed 's/.*GPUI_METRIC /GPU /'
}
real_use_matrix one
python3 -I - "$ROOT" <<'PY'
import glob, json, os, re, sys
root = sys.argv[1]; rows = {}; cols = []
for log in sorted(glob.glob(os.path.join(root, "*-bp*.log"))):
    run = os.path.basename(log)[:-4]; r = rows.setdefault(run, {})
    for l in open(log, errors="ignore"):
        m = re.match(r"PERF (\S+) (\d+)ms", l)
        if m: r[m[1]] = int(m[2])
        for k, v in re.findall(r"(save_\w+_ms)=(-?\d+)", l): r[k] = int(v)
    for k in r:
        if k not in cols: cols.append(k)
print("| run | " + " | ".join(cols) + " |"); print("|---" * (len(cols) + 1) + "|")
for run, r in rows.items(): print(f"| {run} | " + " | ".join(str(r.get(c, "")) for c in cols) + " |")
json.dump(rows, open(os.path.join(root, "baseline.json"), "w"), indent=1)
PY
echo "artifacts: $ROOT"
