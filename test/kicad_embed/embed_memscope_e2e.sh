#!/bin/bash
# The embedded KiCad in a memory scope of its own (native/host/src/memscope.rs `wrap_helper`; Linux with a user systemd), for real:
#  - KiCad runs in a bp-kicad-*.scope in bpkicad.slice with its cap (BACKPLANE_KICAD_MEM_MAX, 12G) and an oom_score_adj above the hub's;
#  - its scope's cap is then lowered under what it uses: the kernel kills KiCad alone, the hub answers throughout and is the same process,
#    and the hub log says KiCad was killed for memory.
# The e2e hub normally has no user bus (XDG_RUNTIME_DIR=$OUT/run): this one gets the person's bus linked in, and its agents' scopes go in a
# test slice (BACKPLANE_SCOPE_SLICE), so nothing of the person's slices is capped by it.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_memscope_e2e: fork only"; exit 0; }
RUN=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
[ -S "$RUN/bus" ] && command -v systemd-run >/dev/null || { echo "embed_memscope_e2e: SKIP (no user systemd)"; exit 0; }
discover_bins
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
mkdir -p "$OUT/run"; ln -sf "$RUN/bus" "$OUT/run/bus"; [ -e "$RUN/systemd" ] && ln -sfn "$RUN/systemd" "$OUT/run/systemd"
export BACKPLANE_SCOPE_SLICE=bpmemscopetest.slice
start_xvfb_and_wm
start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 15000}, {"mark": "ready"}, {"kicad": {"report": True, "tag": "ready"}}, {"wait": 60000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 100 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 600); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done; sleep 1
grep -a "memscope" "$OUT/hub.log" | head -2
HUB=$(pgrep -f -- "--home $OUT/home --port $HPORT" | head -1)
KPID=$(grep -ao '"kicad_info":"[^"]*","gone":false,"state":"ready","mode":"fork","pid":[0-9]*' "$OUT/client.log" | tail -1 | grep -o '[0-9]*$')
CG=$(cut -d: -f3 "/proc/$KPID/cgroup" 2>/dev/null || true); UNIT=$(basename "$CG")
check "KiCad runs in a memory scope of its own in bpkicad.slice" "$(echo "$CG" | grep -q '/bpkicad.slice/bp-kicad-.*\.scope$' && echo 1 || echo 0)" "pid $KPID cgroup $CG"
MAXB=$(cat "/sys/fs/cgroup$CG/memory.max" 2>/dev/null || echo 0)
check "its cap is BACKPLANE_KICAD_MEM_MAX (12G)" "$([ "$MAXB" = "$((12 << 30))" ] && echo 1 || echo 0)" "memory.max $MAXB"
HA=$(cat "/proc/$HUB/oom_score_adj"); KA=$(cat "/proc/$KPID/oom_score_adj")
check "KiCad's oom_score_adj is above the hub's" "$([ "$KA" -gt "$HA" ] && echo 1 || echo 0)" "hub $HA kicad $KA"
CUR=$(cat "/sys/fs/cgroup$CG/memory.current"); LOW=$(( CUR / 2 ))
echo "KiCad's scope uses $((CUR >> 20)) MB; its cap lowered to $((LOW >> 20)) MB"
systemctl --user set-property --runtime "$UNIT" MemoryMax="$LOW" MemorySwapMax=0
UP=1; for _ in $(seq 40); do curl -sf -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" || UP=0; kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
check "the kernel killed KiCad for memory" "$(kill -0 "$KPID" 2>/dev/null && echo 0 || echo 1)" "kicad pid $KPID"
sleep 1.5
check "the hub answered throughout and is the same process" "$([ "$UP" = 1 ] && kill -0 "$HUB" 2>/dev/null && curl -sf -m 2 -o /dev/null "http://127.0.0.1:$HPORT/hello" && echo 1 || echo 0)" "hub $HUB up=$UP"
check "the hub log says KiCad was killed for memory" "$(grep -aq 'memscope: KiCad killed for memory' "$OUT/hub.log" && echo 1 || echo 0)" "$(grep -a 'memscope' "$OUT/hub.log" | tail -2 | tr '\n' ' ')"
systemctl --user stop bpmemscopetest.slice 2>/dev/null || true
finish_suite "embed_memscope_e2e ($EMBED_MODE)"
