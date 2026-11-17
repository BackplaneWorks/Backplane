#!/bin/bash
# DRC while another live KiCad holds the schematic (the person's own KiCad, say) and the editor's KiCad has the board only: KiCad's own
# DRC would load the schematic for its parity check and bp.13 opens it over that lock without asking (lock_overridden; the integrator's
# run, Nov 16). So the hub runs that DRC with kicad-cli on the saved files instead:
#   D1  DRC asked (the person's Run): it runs on kicad-cli (the hub says why), never in KiCad; no lock override; the other KiCad keeps
#       its lock; the editor's KiCad still up
# fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_drc_held_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
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
holders() { lslocks -n -o PID,PATH 2>/dev/null | awk -v p="$LCK" '$2 == p {print $1}' | sort -u | tr '\n' ' '; }
# no pre-warm (its own lock check is another row's): only DRC could open the schematic here
export BACKPLANE_KICAD_PREWARM_FREE_MB=99999999
start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":120000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL the board never opened in KiCad"; exit 1; }
sleep 6
hub_rpc "[{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\",\"source\":\"drc\"}}]" >>"$OUT/rpc.log" 2>&1 || true
sleep 20
CLI=$(grep -c "checks: drc with kicad-cli: the schematic is held by\|has no embed-checks: kicad-cli" "$OUT/hub.log" || true)
INK=$(grep -c "checks: drc in KiCad starts" "$OUT/hub.log" || true)
OVR=$(grep -c "KiCad opened a locked file anyway\|lock_overridden" "$OUT/hub.log" || true)
ENDED=$(grep -c 'kicad embed: .* ended (' "$OUT/hub.log" || true)
echo "INFO checks: $(grep -o 'checks: drc [^(]*' "$OUT/hub.log" | sort | uniq -c | tr '\n' '|' | cut -c1-300)"
check "D1 DRC with the schematic held elsewhere runs on kicad-cli, never in KiCad; no lock override; the other KiCad keeps its lock; KiCad up" "$([ "${CLI:-0}" -ge 1 ] && [ "${INK:-0}" = 0 ] && [ "${OVR:-0}" = 0 ] && [ "$(holders | tr -d ' ')" = "$HOLDER" ] && [ "${ENDED:-0}" = 0 ] && echo 1 || echo 0)" "kicad-cli $CLI, in KiCad $INK, overrides $OVR, held by $(holders), ended $ENDED"
kill "$CP" 2>/dev/null || true
finish_suite "embed_drc_held_e2e ($EMBED_MODE)"
