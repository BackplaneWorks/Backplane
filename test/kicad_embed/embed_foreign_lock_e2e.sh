#!/bin/bash
# Another live KiCad has the project open (its lock ~<name>.kicad_pro.lck held: here a stand-in process that writes KiCad's lock JSON and
# holds its flock, as KiCad does): Edit never opens over it silently (bp.13 would) nor stalls on KiCad's own "File Open Warning" outside
# the pane (bp.12.4 would). The pane asks, Open anyway or Cancel (locks.rs, embed.rs lock_gate):
#   L1  the pane asks (kicad:lock-open, kicad:lock-cancel drawn), no KiCad started for the project, no File Open Warning anywhere
#   L2  Cancel: no KiCad starts; the pane offers Try again; the other KiCad's lock is untouched (still held by it)
#   L3  Try again asks again; Open anyway: the editor paints, no File Open Warning, the project's lock is now held by our KiCad
#   L4  a stale lock (its holder gone) is cleared without asking, and the hub log says so
#   L5  asked, and the other KiCad closes before anyone answers: the question ends by itself, the editor comes
# fork only; under i3 and mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_foreign_lock_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
LCK="$(dirname "$PRO")/~$(basename "$PRO").lck"
HOST=$(hostname | cut -d. -f1)
# the other KiCad: writes the lock as KiCad does and holds its flock until killed
python3 - "$LCK" "$(id -un)" "$HOST" "$OUT/holder.pid" <<'PY' &
import fcntl, json, os, sys, time
lck, user, host, pidf = sys.argv[1:5]
f = open(lck, "w"); json.dump({"hostname": host, "username": user, "token": "other"}, f); f.flush()
fcntl.flock(f.fileno(), fcntl.LOCK_EX)
open(pidf, "w").write(str(os.getpid()))
time.sleep(3600)
PY
HOLDER=$!; track_pid "$HOLDER"
for _ in $(seq 50); do [ -s "$OUT/holder.pid" ] && break; sleep 0.1; done
holders() { lslocks -n -u -o PID,PATH 2>/dev/null | awk -v p="$(readlink -f "$LCK")" '$2 == p {print $1}' | sort -u | tr '\n' ' '; }
echo "INFO the other KiCad (stand-in pid $HOLDER) holds $LCK: $(holders)"
start_xvfb_and_wm; start_hub
HUB_PID=${OWN_GROUPS[${#OWN_GROUPS[@]}-1]}
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"wait":110000},{"mark":"done"}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the latest drawn rect of each id, window coordinates
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
press() { local at; at=$(rect "$1"); [ -n "$at" ] || return 1; read -r X Y <<<"$at"; python3 "$XINPUT_PY" click "$X" "$Y"; }
kicads() { { pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid $HUB_PID([[:space:]]|$)" || true; } | wc -l; }
dlg() { xwininfo -root -tree 2>/dev/null | grep -c '"File Open Warning"' || true; }
# L1
ASKED=0; wait_rect kicad:lock-open 60 && wait_rect kicad:lock-cancel 5 && ASKED=1
sleep 1
check "L1 the pane asks Open anyway or Cancel; no KiCad started for the project, no File Open Warning" "$([ $ASKED = 1 ] && [ "$(kicads)" = 0 ] && [ "$(dlg)" = 0 ] && echo 1 || echo 0)" "asked $ASKED, KiCads $(kicads), dialogs $(dlg); hub: $(grep -o 'kicad embed: .* the person is asked.*' "$OUT/hub.log" | head -1 | cut -c1-200)"
# L2 Cancel
press kicad:lock-cancel || true; sleep 2
RETRY=0; wait_rect kicad:retry 10 && RETRY=1
check "L2 Cancel: no KiCad starts, the pane offers Try again, the other KiCad keeps its lock" "$([ $RETRY = 1 ] && [ "$(kicads)" = 0 ] && [ "$(holders | tr -d ' ')" = "$HOLDER" ] && echo 1 || echo 0)" "retry $RETRY, KiCads $(kicads), lock held by $(holders)"
# L3 Try again, then Open anyway
press kicad:retry || true
ASKED2=0; wait_rect kicad:lock-open 30 && ASKED2=1
press kicad:lock-open || true
PAINTED=0
for _ in $(seq 240); do grep -q 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" 2>/dev/null && { PAINTED=1; break; }; sleep 0.5; done
sleep 3
OURS=$(pgrep -n -u "$(id -u)" -f -- "--embedded.*--parent-pid $HUB_PID([[:space:]]|$)" 2>/dev/null || true)
NOW=$(holders)
check "L3 Try again asks again; Open anyway opens the editor, no File Open Warning, our KiCad holds the project's lock" "$([ $ASKED2 = 1 ] && [ $PAINTED = 1 ] && [ -n "$OURS" ] && [ "$(dlg)" = 0 ] && echo " $NOW " | grep -q " $OURS " && echo 1 || echo 0)" "asked again $ASKED2, ready $PAINTED, dialogs $(dlg), lock held by $NOW (our KiCad $OURS); hub: $(grep -o 'kicad locks: the person chose Open anyway.*' "$OUT/hub.log" | head -1 | cut -c1-200)"
kill "$CP" 2>/dev/null || true
# L4: a stale lock (the stand-in killed, its lock file left) on a second project's open: cleared, logged, never asked
kill "$HOLDER" 2>/dev/null || true
# our KiCad ends as a crash would (kill -9: its lock left behind, held by nobody)
for p in $NOW; do kill -9 "$p" 2>/dev/null || true; done
for _ in $(seq 40); do [ -z "$(holders | tr -d ' ')" ] && break; sleep 0.25; done
sleep 2
python3 -c 'import json,sys; json.dump({"hostname": sys.argv[2], "username": sys.argv[3], "token": "dead"}, open(sys.argv[1], "w"))' "$LCK" "$HOST" "$(id -un)"
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":2000}]' >"$OUT/script2.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script2.json" --exit-after 120 >"$OUT/client2.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client2.log" '"name":"ready","event":"mark"' 600 || true
STALE=$(grep -c "kicad locks: removed the stale lock .*$(basename "$LCK")" "$OUT/hub.log" || true)
check "L4 a stale lock is cleared without asking, and the hub log says so" "$([ "${STALE:-0}" -ge 1 ] && grep -q '"name":"ready","event":"mark"' "$OUT/client2.log" && [ "$(dlg)" = 0 ] && echo 1 || echo 0)" "logged $STALE, dialogs $(dlg)"
kill "$CP" 2>/dev/null || true
# L5: asked, then the other KiCad closes before anyone answers: the question ends by itself and the editor comes
for p in $(holders); do kill -9 "$p" 2>/dev/null || true; done
for _ in $(seq 40); do [ -z "$(holders | tr -d ' ')" ] && break; sleep 0.25; done; sleep 2
python3 - "$LCK" "$(id -un)" "$HOST" "$OUT/holder5.pid" <<'PY' &
import fcntl, json, os, sys, time
lck, user, host, pidf = sys.argv[1:5]
f = open(lck, "w"); json.dump({"hostname": host, "username": user, "token": "other5"}, f); f.flush()
fcntl.flock(f.fileno(), fcntl.LOCK_EX)
open(pidf, "w").write(str(os.getpid()))
time.sleep(3600)
PY
H5=$!; track_pid "$H5"
for _ in $(seq 50); do [ -s "$OUT/holder5.pid" ] && break; sleep 0.1; done
READY0=$(grep -c 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" || true)
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"wait":90000}]' >"$OUT/script3.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script3.json" --exit-after 120 >"$OUT/client3.log" 2>&1 &
CP=$!; track_pid "$CP"
ASK0=2; ASK5=0; for _ in $(seq 240); do [ "$(grep -c 'the person is asked' "$OUT/hub.log" || true)" -gt "$ASK0" ] && { ASK5=1; break; }; sleep 0.25; done; sleep 2
kill "$H5" 2>/dev/null || true
T5=$(date +%s%3N); R5=-1
for _ in $(seq 120); do [ "$(grep -c 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" || true)" -gt "$READY0" ] && { R5=$(( $(date +%s%3N) - T5 )); break; }; sleep 0.25; done
check "L5 the other KiCad closes while the pane asks: the question ends by itself and the editor comes" "$([ $ASK5 = 1 ] && [ "$R5" -ge 0 ] && grep -q 'let go of .*the question ends by itself' "$OUT/hub.log" && echo 1 || echo 0)" "asked $ASK5, editor ready $R5 ms after the other closed"
kill "$CP" 2>/dev/null || true
# L6: a lock another computer wrote (a shared or synced folder), held by nobody here (no flock across machines): never removed, the person asked
for p in $(holders); do kill -9 "$p" 2>/dev/null || true; done
for _ in $(seq 40); do [ -z "$(holders | tr -d ' ')" ] && break; sleep 0.25; done; sleep 2
python3 -c 'import json,sys; json.dump({"hostname": "another-computer", "username": "colleague", "token": "far"}, open(sys.argv[1], "w"))' "$LCK"
ASK0=$(grep -c 'the person is asked' "$OUT/hub.log" || true); RM0=$(grep -c "kicad locks: removed the stale lock .*$(basename "$LCK")" "$OUT/hub.log" || true)
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"wait":30000}]' >"$OUT/script6.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script6.json" --exit-after 60 >"$OUT/client6.log" 2>&1 &
CP=$!; track_pid "$CP"
ASK6=0; for _ in $(seq 240); do [ "$(grep -c 'the person is asked' "$OUT/hub.log" || true)" -gt "$ASK0" ] && { ASK6=1; break; }; sleep 0.25; done; sleep 2
RM6=$(( $(grep -c "kicad locks: removed the stale lock .*$(basename "$LCK")" "$OUT/hub.log" || true) - RM0 ))
check "L6 another computer's lock nobody holds here is never removed: the person is asked who has it" "$([ $ASK6 = 1 ] && [ -e "$LCK" ] && [ "$RM6" = 0 ] && grep -q 'colleague at another-computer' "$OUT/hub.log" && echo 1 || echo 0)" "asked $ASK6, lock file still there $([ -e "$LCK" ] && echo yes || echo no), removed $RM6; hub: $(grep -o 'the person is asked.*' "$OUT/hub.log" | tail -1 | cut -c1-200)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_foreign_lock_e2e ($EMBED_MODE)"
