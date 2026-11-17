#!/bin/bash
# bp.13 opens a file another process holds without asking (embedded, nobody could answer) and says so (ET_AGENT_NOTICE lock_overridden).
# The hub's lock checks come first; this is the net for one they did not see: never silent. Here the pre-warm of the schematic, while a
# stand-in holds its lock, is that open (LOCK_NOTICE_TRIGGER=prewarm: a runtime whose pre-warm has no lock check of its own):
#   N1  the hub logs KiCad's notice, and the strip draws it (kicad:notice) with its words
# fork only; bp.13 and later (an older runtime asks in its own box instead, which this row does not judge: SKIP).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_lock_notice_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; discover_kicad; init_isolated_dirs
VER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("version",""))' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null || echo "")
case "$VER" in *bp.1[3-9]*|*bp.[2-9][0-9]*) ;; *) echo "SKIP embed_lock_notice_e2e: $VER asks in its own box (no lock_overridden)"; exit 0 ;; esac
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
SCH=${PRO%.kicad_pro}.kicad_sch; LCK="$(dirname "$SCH")/~$(basename "$SCH").lck"
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
start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":60000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 120 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL the board never opened in KiCad"; exit 1; }
# the pre-warm of the schematic (or DRC's parity) opens it over the held lock; KiCad says so
for _ in $(seq 120); do grep -q "KiCad notice (lock_overridden)" "$OUT/hub.log" && break; sleep 0.5; done
sleep 4
LOGGED=$(grep -c "KiCad notice (lock_overridden)" "$OUT/hub.log" || true)
DRAWN=$(grep -c '"id":"kicad:notice"' "$OUT/client.log" || true)
echo "INFO $(grep -o 'KiCad notice (lock_overridden).*' "$OUT/hub.log" | head -1 | cut -c1-220)"
if [ "${LOGGED:-0}" = 0 ]; then echo "SKIP N1: nothing opened the schematic over the lock here (the hub's own checks kept it shut): no notice to judge"; else
check "N1 KiCad's lock override is said: the hub's notice, and the strip draws it" "$([ "${DRAWN:-0}" -ge 1 ] && echo 1 || echo 0)" "logged $LOGGED, drawn $DRAWN"
fi
kill "$CP" 2>/dev/null || true
finish_suite "embed_lock_notice_e2e ($EMBED_MODE)"
