#!/bin/bash
# A BOM edit while the editor's KiCad has the project but not the schematic (the board shown, nothing pre-warmed): it goes through the
# editor's own KiCad, which opens the schematic out of sight (embed::open_hidden), never through a headless KiCad beside it (that one
# would hold the project's and the schematic's locks: bp.13 opens over them without asking, bp.12.4 asks outside the pane):
#   H1  the edit answers ok, made in KiCad ("via": "kicad"); no headless KiCad started for the project (no "kicad ipc: … up")
#   H2  KiCad has the schematic open now (out of sight: the pane still shows the board), and one KiCad holds the schematic's lock
#   H3  an agent's kicad_ipc through the headless path is never started beside the editor (ipc::HEADLESS_BESIDE_EDITOR) (unit-tested;
#       here: still no headless KiCad after the edit)
# fork only; under i3 and mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_bom_editor_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
SCH=${PRO%.kicad_pro}.kicad_sch; LCK="$(dirname "$SCH")/~$(basename "$SCH").lck"
REF=$(grep -o '(property "Reference" "[A-Z]\+[0-9]\+"' "$SCH" | head -1 | grep -o '"[A-Z]\+[0-9]\+"' | tr -d '"')
# nothing pre-warmed: the schematic is not open in KiCad when the edit comes
export BACKPLANE_KICAD_PREWARM_FREE_MB=99999999
start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":120000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL the board never opened in KiCad"; exit 1; }
sleep 5
headless() { grep -c "kicad ipc: .* up" "$OUT/hub.log" || true; }
SCH_BEFORE=$(grep -o 'publish editor.kicad[^ ]* .*' "$OUT/hub.log" | tail -1 | grep -c '"kind":"SCH"' || true)
echo "INFO the schematic open in KiCad before the edit: $SCH_BEFORE (pre-warm off); BOM reference $REF"
# KiCad's own automatic DRC may hold its task gate for a moment (the panel says "a task is running": asked again, as the person would)
for _ in 1 2 3 4; do
  R=$(hub_rpc "[{\"method\":\"mgmt.bom.edit\",\"params\":{\"project\":\"$PRO\",\"field\":\"mpn\",\"refs\":[\"$REF\"],\"value\":\"E2E-EDITOR\"}}]" 2>&1 | grep -a '"mgmt.bom.edit"' | head -1 | cut -c1-400 || true)
  echo "$R" | grep -q 'a task is running' || break; sleep 5
done
echo "INFO the edit: $R"
sleep 3
check "H1 the BOM edit goes through the editor's KiCad; no headless KiCad started" "$(echo "$R" | grep -q '"ok":true' && echo "$R" | grep -q '"via":"kicad"' && [ "$(headless)" = 0 ] && echo 1 || echo 0)" "headless starts $(headless)"
HOLDERS=$(lslocks -n -o PID,PATH 2>/dev/null | awk -v p="$LCK" '$2 == p {print $1}' | sort -u | tr '\n' ' ')
KP=$(pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid" -n 2>/dev/null || true)
check "H2 the editor's KiCad has the schematic (opened out of sight, or by its DRC parity) and holds its lock, alone" "$([ "$(echo $HOLDERS | wc -w)" = 1 ] && echo " $HOLDERS " | grep -q " $KP " && echo 1 || echo 0)" "lock held by $HOLDERS (the editor's KiCad $KP)"
sleep 5
check "H3 still no headless KiCad beside the editor after the edit" "$([ "$(headless)" = 0 ] && echo 1 || echo 0)" "$(headless)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_bom_editor_e2e ($EMBED_MODE)"
