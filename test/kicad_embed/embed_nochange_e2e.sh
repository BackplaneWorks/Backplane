#!/bin/bash
# What nobody changed stays byte for byte (runtimes with no-load-migration: bp.12.3, bp.13 batch 7): the project opened in the embedded
# editor (board and schematic), the hub's checks run in it (DRC and ERC), then both documents saved as they are, as a person's Ctrl+S
# would: every board and sheet file byte-identical, KiCad saying unmodified throughout. On an older runtime it says what changed (KNOWN).
# KEMBED_PROJ_SRC the project (default Backplane's ecc83-pp fixture). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_nochange_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1); DIR=$(dirname "$PRO")
NLM=0; grep -q '"no-load-migration"' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null && NLM=1
sums() { (cd "$DIR" && find . -name '*.kicad_sch' -o -name '*.kicad_pcb' | sort | xargs sha256sum | cut -c1-16 | tr '\n' ' '); }
S0=$(sums)
start_xvfb_and_wm; start_hub
cat > "$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"e"}},{"click":"kicad-tab:SCH"},{"kicad":{"wait_painted":120000,"kind":"SCH","tag":"s"}},{"wait":2000},{"mark":"open"},{"wait":150000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 220 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 2400); do grep -q '"name":"open","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" s); SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock",""))' "$R")
mod() { python3 "$IPC_PY" modified "$SOCK" "$1" 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?"; }
echo "INFO after open: board modified $(mod pcb), schematic modified $(mod sch) (no-load-migration: $NLM)"
S1=$(sums)
# the hub's checks in the open editor
hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}},{\"method\":\"kicad.checks.run\",\"params\":{\"project\":\"$PRO\"}}]" >/dev/null 2>&1 || true
for _ in $(seq 240); do if grep -q 'checks: erc' "$OUT/hub.log" && grep -q 'checks: drc' "$OUT/hub.log"; then break; fi; sleep 0.5; done; sleep 8
grep -E '^checks: (drc|erc)' "$OUT/hub.log" | head -4 | sed 's/^/INFO /' || true
echo "INFO after the checks: board modified $(mod pcb), schematic modified $(mod sch)"
S2=$(sums)
# saved as they are (a person's Ctrl+S on each)
python3 "$IPC_PY" save-doc "$SOCK" sch >/dev/null 2>&1 || true; python3 "$IPC_PY" save-doc "$SOCK" pcb >/dev/null 2>&1 || true; sleep 2
S3=$(sums)
if [ "$NLM" = 1 ]; then
  check "opening leaves every board and sheet file byte-identical" "$([ "$S1" = "$S0" ] && echo 1 || echo 0)" "$S0 -> $S1"
  check "the checks leave them byte-identical" "$([ "$S2" = "$S0" ] && echo 1 || echo 0)" "$S0 -> $S2"
  check "an unmodified save writes nothing" "$([ "$S3" = "$S0" ] && echo 1 || echo 0)" "$S0 -> $S3"
  check "KiCad says both unmodified after it all" "$([ "$(mod pcb)" = False ] && [ "$(mod sch)" = False ] && echo 1 || echo 0)" "board $(mod pcb), schematic $(mod sch)"
else
  echo "KNOWN this runtime has no no-load-migration: files after open/checks/save: $([ "$S3" = "$S0" ] && echo unchanged || echo changed)"
fi
kill "$CP" 2>/dev/null || true
finish_suite "embed_nochange_e2e ($EMBED_MODE)"
