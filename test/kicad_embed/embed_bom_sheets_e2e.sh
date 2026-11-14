#!/bin/bash
# A BOM edit lands for a symbol wherever it sits (bomedit.rs): flat sheet, a sub-sheet used once (Hubble_DK: sheets/*.kicad_sch), a
# sub-sheet used twice (the fork's complex_hierarchy: ampli_ht.kicad_sch as RV201 and RV301). It is never said saved unless the saved
# file shows it; a symbol of a sheet used twice has one value for both instances (KiCad's model), and the answer names the other.
# KEMBED_PROJ_SRC the project (default: Backplane's ecc83-pp fixture); BOM_REFS "REF[ REF2]" the references to edit (default R1; with
# two, the second is the first's other instance: its edit must reach both). First through the headless KiCad (no editor open: saved),
# then in the open editor (Edit, Schematic tab: in KiCad, unsaved). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_bom_sheets_e2e: fork only"; exit 0; }
# BOM_CASE=twice: the fork's complex_hierarchy demo (ampli_ht.kicad_sch used as RV201 and RV301), where it is checked out
if [ "${BOM_CASE:-}" = twice ]; then
  CH=${BOM_CH_SRC:-$REAL_HOME/Documents/Backplane/Backplane_KiCad/demos/complex_hierarchy}
  [ -d "$CH" ] || { echo "SKIP embed_bom_sheets_e2e: no complex_hierarchy demo at $CH"; exit 0; }
  export KEMBED_PROJ_SRC=$CH BOM_REFS="RV201 RV301"
fi
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1); DIR=$(dirname "$PRO")
read -r -a REFS <<< "${BOM_REFS:-R1}"
export BACKPLANE_KICAD_WARM=0
start_xvfb_and_wm; start_hub
edit() { hub_rpc "[{\"method\":\"mgmt.bom.edit\",\"params\":{\"project\":\"$PRO\",\"field\":\"mpn\",\"refs\":[\"$1\"],\"value\":\"$2\"}}]" 2>&1 | grep -a '^{"m":"mgmt.bom.edit"' | head -1 || true; }
shows() { python3 - "$DIR" "$1" "$2" <<'PY'
import glob, os, re, sys
d, ref, val = sys.argv[1:4]
for f in glob.glob(os.path.join(d, "**", "*.kicad_sch"), recursive=True):
    t = open(f, errors="ignore").read()
    for m in re.finditer(r'\n\t\(symbol\n\t\t\(lib_(id|name)', t):
        i = m.start() + 1; depth = 0; j = i
        while j < len(t):
            c = t[j]
            if c == '(': depth += 1
            elif c == ')':
                depth -= 1
                if depth == 0: break
            j += 1
        b = t[i:j]
        if f'(reference "{ref}")' in b and f'(property "MPN" "{val}"' in b: print(os.path.relpath(f, d)); sys.exit(0)
print(""); sys.exit(1)
PY
}
R1=${REFS[0]}
A=$(edit "$R1" "E2E-A"); echo "INFO headless $R1: $(echo "$A" | cut -c1-260)"
F=$(shows "$R1" E2E-A || true)
check "headless: $R1's MPN is in the saved file ($F)" "$([ -n "$F" ] && echo "$A" | grep -q '"via":"file"' && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
if [ ${#REFS[@]} -gt 1 ]; then
  R2=${REFS[1]}
  check "headless: the answer names the other instance ($R2) as sharing it" "$(echo "$A" | grep -q "\"shared_with\":\\[\"$R2\"\\]" && echo 1 || echo 0)" "$(echo "$A" | grep -o '"shared_with":[^]]*]' | head -1)"
  B=$(edit "$R2" "E2E-B"); echo "INFO headless $R2: $(echo "$B" | cut -c1-200)"
  check "headless: $R2's edit is saved, and $R1 (one symbol) shows it too" "$( shows "$R2" E2E-B >/dev/null && shows "$R1" E2E-B >/dev/null && echo 1 || echo 0)"
fi
N=$(edit "Q999" "x"); check "a reference that is nowhere is refused, naming it" "$(echo "$N" | grep -q 'no symbol Q999' && echo 1 || echo 0)" "$(echo "$N" | cut -c1-200)"
# in the open editor
cat > "$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"e"}},{"click":"kicad-tab:SCH"},{"kicad":{"wait_painted":120000,"kind":"SCH","tag":"s"}},{"wait":2000},{"mark":"ready"},{"wait":60000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 160 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 2400); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; sleep 0.1; done
G=$(edit "$R1" "E2E-C"); echo "INFO in KiCad $R1: $(echo "$G" | cut -c1-260)"
check "in KiCad: $R1 changed (KiCad read back), not claimed saved" "$(echo "$G" | grep -q '"ok":true' && echo "$G" | grep -q '"via":"kicad"' && ! echo "$G" | grep -q '"changed":0' && echo 1 || echo 0)" "$(echo "$G" | cut -c1-240)"
[ ${#REFS[@]} -gt 1 ] && check "in KiCad: the answer names ${REFS[1]} as sharing it" "$(echo "$G" | grep -q "\"shared_with\":\\[\"${REFS[1]}\"\\]" && echo 1 || echo 0)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_bom_sheets_e2e ($EMBED_MODE, ${REFS[*]})"
