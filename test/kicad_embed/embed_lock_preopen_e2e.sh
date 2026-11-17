#!/bin/bash
# A document KiCad opens after its start (the schematic, the board open first) whose lock another live KiCad holds: never opened over
# that lock without asking (bp.13 would, bp.12.4 would ask outside the pane), never by a headless KiCad beside the editor's:
#   P1  the schematic is not pre-warmed while another KiCad holds it (the hub log says why); no lock_overridden notice
#   P2  a BOM edit with the editor's KiCad on the board only: refused, naming the holder; no headless KiCad started for the project
#   P3  the person's switch to the schematic asks (Open anyway / Cancel); Open anyway: the schematic's editor is shown, no File Open
#       Warning, our KiCad holds the schematic's lock
#   P4  a BOM edit now goes through the editor's KiCad (no headless KiCad), and the saved schematic shows it
# fork only; under i3 and mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_lock_preopen_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
SCH=${PRO%.kicad_pro}.kicad_sch
[ -f "$SCH" ] || { echo "FAIL the fixture has no schematic beside $PRO"; exit 1; }
LCK="$(dirname "$SCH")/~$(basename "$SCH").lck"
REF=$(grep -o '(property "Reference" "[A-Z]\+[0-9]\+"' "$SCH" | head -1 | grep -o '"[A-Z]\+[0-9]\+"' | tr -d '"')
python3 - "$LCK" "$(id -un)" "$(hostname | cut -d. -f1)" "$OUT/holder.pid" <<'PY' &
import fcntl, json, os, sys, time
lck, user, host, pidf = sys.argv[1:5]
f = open(lck, "w"); json.dump({"hostname": host, "username": user, "token": "other"}, f); f.flush()
fcntl.flock(f.fileno(), fcntl.LOCK_EX)
open(pidf, "w").write(str(os.getpid()))
time.sleep(3600)
PY
HOLDER=$!; track_pid "$HOLDER"
for _ in $(seq 50); do [ -s "$OUT/holder.pid" ] && break; sleep 0.1; done
holders() { lslocks -n -u -o PID,PATH 2>/dev/null | awk -v p="$(readlink -f "$1")" '$2 == p {print $1}' | sort -u | tr '\n' ' '; }
echo "INFO another KiCad (stand-in pid $HOLDER) holds $LCK; the BOM edit's reference: $REF"
# the hub logs KiCad's events (P3 reads its "schematic shown")
export BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub
HUB_PID=${OWN_GROUPS[${#OWN_GROUPS[@]}-1]}
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":420000},{"mark":"done"}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 480 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL the board never opened in KiCad"; exit 1; }
sleep 8
headless() { grep -c "kicad ipc: .* up" "$OUT/hub.log" || true; }
OVR() { grep -c "KiCad opened a locked file anyway" "$OUT/hub.log" || true; }
# P1: DRC asked now (its parity would load the schematic in KiCad): it must go to kicad-cli while the schematic is held, never open it
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\",\"source\":\"drc\"}}]" >>"$OUT/rpc.log" 2>&1 || true
sleep 15
CLI_DRC=$(grep -c "checks: drc with kicad-cli: the schematic is held by\|has no embed-checks: kicad-cli" "$OUT/hub.log" || true)
echo "INFO P1 checks: $(grep -o 'checks: \(drc\|erc\) \(in KiCad starts\|with kicad-cli[^(]*\)' "$OUT/hub.log" | tr '\n' '|' | cut -c1-300)"
check "P1b DRC while the schematic is held elsewhere runs on kicad-cli (KiCad's parity would open it); no lock override" "$([ "${CLI_DRC:-0}" -ge 1 ] && [ "$(OVR)" = 0 ] && echo 1 || echo 0)" "kicad-cli DRC for the held schematic $CLI_DRC, overrides $(OVR)"
SKIP=$(grep -c "not pre-warming SCH: .*held by" "$OUT/hub.log" || true)
check "P1 the schematic is not pre-warmed while another KiCad holds it; no lock override" "$([ "${SKIP:-0}" -ge 1 ] && [ "$(OVR)" = 0 ] && [ "$(holders "$LCK" | tr -d ' ')" = "$HOLDER" ] && echo 1 || echo 0)" "skipped $SKIP, overrides $(OVR), held by $(holders "$LCK")"
# P2
edit() { hub_rpc "[{\"method\":\"mgmt.bom.edit\",\"params\":{\"project\":\"$PRO\",\"field\":\"mpn\",\"refs\":[\"$REF\"],\"value\":\"$1\"}}]" 2>&1 | grep -a '"mgmt.bom.edit"\|"error"\|"ok"' | head -1 | cut -c1-400 || true; }
R2=$(edit "E2E-HELD")
echo "INFO P2 reply: $R2"
check "P2 a BOM edit with the schematic held elsewhere is refused naming the holder; no headless KiCad" "$(echo "$R2" | grep -q 'is open in' && [ "$(headless)" = 0 ] && ! grep -q 'E2E-HELD' "$SCH" && echo 1 || echo 0)" "$R2; headless starts $(headless)"
# P3: the switch (the script's click at its "switch" mark) asks
rect() { python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
last = None; ids = set()
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"rects":[' in l:
        try: rs = json.loads(l.split("GPUI_METRIC ", 1)[1])["rects"]
        except Exception: continue
        if isinstance(rs, list) and any(r.get("id", "").startswith("kicad:") for r in rs):
            ids = {r["id"] for r in rs}; last = next((r for r in rs if r.get("id") == sys.argv[2]), None)
print("%d %d" % (last["x"] + last["w"] / 2, last["y"] + last["h"] / 2) if last and sys.argv[2] in ids else "")
PY
}
wait_rect() { for _ in $(seq $(( $2 * 4 ))); do [ -n "$(rect "$1")" ] && return 0; sleep 0.25; done; return 1; }
# the person's switch: the pane's own Schematic tab
AT=""; for _ in $(seq 40); do AT=$(rect kicad-tab:SCH); [ -n "$AT" ] && break; sleep 0.25; done
[ -n "$AT" ] && { read -r X Y <<<"$AT"; python3 "$XINPUT_PY" click "$X" "$Y"; }
echo "INFO P3: the Schematic tab at ${AT:-nowhere}"
ASKED=0; wait_rect kicad:lock-open 90 && ASKED=1
AT=$(rect kicad:lock-open); [ -n "$AT" ] && { read -r X Y <<<"$AT"; python3 "$XINPUT_PY" click "$X" "$Y"; }
SHOWN=0
for _ in $(seq 120); do grep -q 'ET_FRAME_SHOWN.*"kind":"FK_SCH"' "$OUT/hub.log" && { SHOWN=1; break; }; sleep 0.5; done
sleep 3
OURS=$(pgrep -n -u "$(id -u)" -f -- "--embedded.*--parent-pid $HUB_PID([[:space:]]|$)" 2>/dev/null || true)
DLG=$(xwininfo -root -tree 2>/dev/null | grep -c '"File Open Warning"' || true)
check "P3 the switch to the schematic asks; Open anyway shows it, no File Open Warning, our KiCad holds its lock" "$([ $ASKED = 1 ] && [ $SHOWN = 1 ] && [ "${DLG:-0}" = 0 ] && echo " $(holders "$LCK") " | grep -q " $OURS " && echo 1 || echo 0)" "asked $ASKED, shown $SHOWN, dialogs $DLG, held by $(holders "$LCK") (ours $OURS); $(grep -o 'kicad locks: the person chose Open anyway.*' "$OUT/hub.log" | head -1 | cut -c1-160)"
# P4
R4=$(edit "E2E-GUI")
echo "INFO P4 reply: $R4"
for _ in $(seq 40); do grep -q 'E2E-GUI' "$SCH" && break; sleep 0.5; done
check "P4 a BOM edit goes through the editor's KiCad, no headless KiCad, the saved schematic shows it" "$(echo "$R4" | grep -q '"ok":true' && [ "$(headless)" = 0 ] && echo 1 || echo 0)" "$R4; headless $(headless); in the file: $(grep -c E2E-GUI "$SCH" || true)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_lock_preopen_e2e ($EMBED_MODE)"
