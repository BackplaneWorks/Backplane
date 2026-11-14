#!/bin/bash
# KiCad's task gate (task-gate-reads, lane P): while a check runs, the hub's frame requests are refused AS_BUSY; the hub sends them again
# when it ends. A tab switch to an editor not yet shown, pressed while a slow DRC runs (the fork's demo `video`, a few seconds), lands
# right after the check; the pane says it waits (never an error); a pane resize meanwhile follows at once (the client's own, no request).
# Prints TIMING lines; KEMBED_PROJ_SRC (default the fork's video demo, copied). fork only; on a runtime without the gate it measures the
# same journey (the requests then wait inside KiCad).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_task_gate_e2e: fork only"; exit 0; }
export KEMBED_PROJ_SRC=${KEMBED_PROJ_SRC:-$REAL_HOME/Documents/Backplane/Backplane_KiCad/demos/video}
# no pre-warm: the schematic's first show is a request KiCad's task gate refuses
export BACKPLANE_KICAD_PREWARM_FREE_MB=999999
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
PRO=$(find "$OUT/proj" -maxdepth 2 -name '*.kicad_pro' -not -path '*/.git/*' | head -1)
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 120000, "kind": "PCB", "tag": "ready"}},
     {"mark": "ready"}, {"wait": 6000}, {"mark": "run"}, {"wait": 700}, {"click": "kicad-tab:SCH"}, {"mark": "switch"}, {"wait": 250}, {"kicad": {"report": True, "tag": "during"}}, {"wait": 350}, {"kicad": {"report": True, "tag": "during"}}, {"wait": 500}, {"kicad": {"report": True, "tag": "during"}},
     {"kicad": {"wait_painted": 120000, "kind": "SCH", "tag": "sch"}}, {"mark": "shown"}, {"wait": 2000}, {"mark": "done"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wm() { for _ in $(seq 2400); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.05; done; return 1; }
wm run && { hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}},{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\",\"source\":\"drc\"}}]" >/dev/null 2>&1 || true; echo "INFO check asked at $(date +%s%3N)"; }
wm switch && for k in 1 2 3 4 5 6; do sleep 0.2; echo "HUBINFO $(hub_rpc "[]" 2>/dev/null | grep -o 'task_wait[^,]*' | head -1)"; done
wm done || true
S=$(last_kicad_report "$OUT/client.log" sch)
SAY=$(python3 - "$OUT/client.log" <<'PY'
import json, sys
says = []
for l in open(sys.argv[1], errors="ignore"):
    if '"tag":"during"' in l and "{" in l:
        try: says.append(json.loads(l[l.index("{"):]).get("say") or "")
        except Exception: pass
print(" | ".join(says))
PY
)
echo "TIMING switch_to_sch_ms $(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("wait_ms"))' "$S")"
grep -E 'checks: drc in KiCad|waits on KiCad.s task|sent after KiCad.s task|kicad embed: request kiapi.backplane.embed.ShowFrame' "$OUT/hub.log" | sed 's/^/HUB /' | head -12
echo "INFO pane said during the check: $SAY"
check "the switch landed (the schematic editor shown)" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(r.get("state")=="ready" and (r.get("wait_ms") or -1) >= 0))' "$S")" "$(echo "$S" | cut -c1-200)"
check "the pane never said the switch failed" "$(echo "$SAY" | grep -qi 'could not\|failed\|error' && echo 0 || echo 1)" "$SAY"
wait "$CP" 2>/dev/null || true
finish_suite "embed_task_gate_e2e ($EMBED_MODE)"
