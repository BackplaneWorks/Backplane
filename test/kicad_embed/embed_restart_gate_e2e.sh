#!/bin/bash
# The editor comes back after whatever ended it, within 30 s, with no KiCad dialog anywhere and one KiCad on the project (0.16.0 gate):
#   W1  warm and Edit together (the thread shown and Edit pressed at once): one KiCad process for the project, painted within 30 s
#   K1  KiCad killed (kill -9) while the editor is up: the pane says KiCad closed unexpectedly and offers Open the editor again (lane J's
#       276b1dd1e); pressed: painted within 30 s, no File Open Warning, one KiCad, the dead KiCad's lock cleared and logged
#   R1  the hub restarts its own KiCad (editor.kicad.restart, as an update does): never asked about its own lock, up within 30 s
#   B1  Backplane killed (hub and window, kill -9) while the editor is up, then started again on the same home: Edit paints within 30 s,
#       no File Open Warning, one KiCad on the project (the old one went with its parent, --parent-pid)
# fork only; under i3 and mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_restart_gate_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
LCK="$(dirname "$PRO")/~$(basename "$PRO").lck"
LIMIT=${GATE_EDIT_S:-30}
start_xvfb_and_wm; start_hub
kicads() { pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid.*$(basename "$PRO")" 2>/dev/null | tr '\n' ' '; }
nkicads() { echo $(kicads) | wc -w; }
dlg() { xwininfo -root -tree 2>/dev/null | grep -c '"File Open Warning"' || true; }
ms() { python3 - "$1" "$2" "$3" <<'PY'
import json, sys
at = {}
for l in open(sys.argv[1], errors="ignore"):
    for n in (sys.argv[2], sys.argv[3]):
        if f'"name":"{n}","event":"mark"' in l and n not in at: at[n] = int(json.loads(l.split("GPUI_METRIC ", 1)[1])["epoch_ms"])
print(at[sys.argv[3]] - at[sys.argv[2]] if len(at) == 2 else -1)
PY
}
rect() { python3 - "$1" "$2" <<'PY'
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
client() { # $1 log, $2 script: a client on the hub now up
  HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$2" --exit-after 400 >"$1" 2>&1 &
  CP=$!; track_pid "$CP"
}
# W1: the thread selected and Edit pressed with no wait between (the warm and Edit's attach race)
echo '[{"select":0},{"click":"rail:Board"},{"mark":"edit"},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":300000}]' >"$OUT/s1.json"
client "$OUT/c1.log" "$OUT/s1.json"
wait_for_log "$OUT/c1.log" '"name":"ready","event":"mark"' 900 || true
sleep 3
W1=$(ms "$OUT/c1.log" edit ready)
check "W1 warm and Edit together: one KiCad for the project, painted within $LIMIT s" "$([ "$W1" -ge 0 ] && [ "$W1" -le $((LIMIT * 1000)) ] && [ "$(nkicads)" = 1 ] && echo 1 || echo 0)" "$W1 ms, KiCads: $(kicads)"
# K1: KiCad killed
OLD=$(kicads); for p in $OLD; do kill -9 "$p" 2>/dev/null || true; done
echo "INFO K1: killed KiCad $OLD"
AT=""; for _ in $(seq 120); do AT=$(rect "$OUT/c1.log" kicad:open-again); [ -n "$AT" ] && break; sleep 0.25; done
[ -n "$AT" ] && { read -r X Y <<<"$AT"; python3 "$XINPUT_PY" click "$X" "$Y"; }
T0=$(date +%s%3N); K1=-1
for _ in $(seq $(( (LIMIT + 30) * 4 ))); do
  if [ "$(grep -c 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log")" -ge 2 ]; then K1=$(( $(date +%s%3N) - T0 )); break; fi; sleep 0.25; done
sleep 3
CLEARED=$(grep -c "kicad locks: removed the stale lock .*$(basename "$LCK")" "$OUT/hub.log" || true)
check "K1 KiCad killed: Open the editor again offered; pressed, the editor is back within $LIMIT s, no File Open Warning, one KiCad" "$([ -n "$AT" ] && [ "$K1" -ge 0 ] && [ "$K1" -le $((LIMIT * 1000)) ] && [ "$(dlg)" = 0 ] && [ "$(nkicads)" = 1 ] && echo 1 || echo 0)" "offered ${AT:+yes}, back in $K1 ms (hub ready), dialogs $(dlg), KiCads: $(kicads); stale locks cleared $CLEARED"
# R1: the hub restarts its own KiCad (editor.kicad.restart, as an update does): the old one is the hub's own until it is gone, so its lock
# is waited for, never asked about (lane K's bp.13 update gate: the person was asked about the KiCad being stopped)
ASKS0=$(grep -c 'the person is asked' "$OUT/hub.log" || true); READY0=$(grep -c 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" || true)
hub_rpc "[{\"method\":\"editor.kicad.restart\",\"params\":{\"project\":\"$PRO\"}}]" >>"$OUT/rpc.log" 2>&1 || true
T0=$(date +%s%3N); R1=-1
for _ in $(seq $(( (LIMIT + 30) * 4 ))); do [ "$(grep -c 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" || true)" -gt "$READY0" ] && { R1=$(( $(date +%s%3N) - T0 )); break; }; sleep 0.25; done
sleep 3
ASKS=$(( $(grep -c 'the person is asked' "$OUT/hub.log" || true) - ASKS0 ))
check "R1 the hub restarts its own KiCad: never asked about its own lock, the new KiCad up within $LIMIT s, one KiCad" "$([ "$ASKS" = 0 ] && [ "$R1" -ge 0 ] && [ "$R1" -le $((LIMIT * 1000)) ] && [ "$(nkicads)" = 1 ] && echo 1 || echo 0)" "asked $ASKS times, up in $R1 ms, KiCads: $(kicads); $(grep -o 'is still held by this hub.s KiCad.*' "$OUT/hub.log" | tail -1 | cut -c1-120)"
# B1: Backplane killed (its hub and its window), started again on the same home
HUBPID=$(pgrep -u "$(id -u)" -f -- "backplane-hub.*--home $OUT/home" | tr '\n' ' ')
kill -9 "$CP" $HUBPID 2>/dev/null || true
for _ in $(seq 40); do [ -z "$(pgrep -u "$(id -u)" -f -- "backplane-hub.*--home $OUT/home")" ] && break; sleep 0.25; done
echo "INFO B1: killed the window $CP and the hub $HUBPID; KiCad then: $(kicads)"
for _ in $(seq 40); do [ "$(nkicads)" = 0 ] && break; sleep 0.25; done
echo "INFO B1: KiCads 10 s after: $(kicads)"
mv "$OUT/hub.log" "$OUT/hub1.log"
start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"mark":"edit"},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":5000}]' >"$OUT/s2.json"
client "$OUT/c2.log" "$OUT/s2.json"
wait_for_log "$OUT/c2.log" '"name":"ready","event":"mark"' 900 || true
sleep 3
B1=$(ms "$OUT/c2.log" edit ready)
check "B1 Backplane killed and started again: Edit paints within $LIMIT s, no File Open Warning, one KiCad" "$([ "$B1" -ge 0 ] && [ "$B1" -le $((LIMIT * 1000)) ] && [ "$(dlg)" = 0 ] && [ "$(nkicads)" = 1 ] && echo 1 || echo 0)" "$B1 ms, dialogs $(dlg), KiCads: $(kicads); $(grep -o 'kicad locks: .*' "$OUT/hub.log" | head -2 | tr '\n' '|' | cut -c1-200)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_restart_gate_e2e ($EMBED_MODE)"
