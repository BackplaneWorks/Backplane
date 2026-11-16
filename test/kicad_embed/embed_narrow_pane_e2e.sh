#!/bin/bash
# The board and the schematic in a narrow pane, as a fresh install opens them: KiCad's side panels (Appearance, Properties, Selection
# Filter; bp.10 on Windows filled a 611 px pane with them, the canvas a sliver) must leave the canvas the pane: after Edit in a pane of
# 611, 480 and 1000 px (NARROW_WIDTHS), with a fresh KiCad profile, the canvas is at least 60 % of the pane's width, in the board editor
# and in the schematic editor; at Full size (1679 px) Appearance and Properties are visible on that fresh profile (autopanels shows its
# columns whenever the canvas keeps 60 % with them: the frame wider than the canvas by a panel's width); Appearance closed by the person
# stays closed after narrow and wide again. A fresh profile is KiCad's defaults (the harness's own seed, which hides the panels for the
# other suites, is taken out). bp.13 (embed-autopanels) is checked; an older runtime's canvas share is said, not checked.
# The canvas is found in the frame's own picture by its background (lib/canvas_box.py): the measure Carbon and lane M use too.
# fork only; under i3 and under mutter (KEMBED_WM).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ -z "${NARROW_W:-}" ]; then rc=0; for w in ${NARROW_WIDTHS:-611 480 1000}; do NARROW_W=$w OUT= bash "$0" "$@" || rc=1; done; exit $rc; fi
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_narrow_pane_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; discover_kicad; init_isolated_dirs
SW=$NARROW_W
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
# a fresh profile as a person's is: KiCad's own panel defaults (seed_project.py hides every side panel for the other suites, so their
# canvases are large: the gate would measure that, not KiCad)
python3 - "$OUT/fh" <<'PY'
import glob, json, sys
for f in glob.glob(sys.argv[1] + "/.config/kicad/*/*.json") + glob.glob(sys.argv[1] + "/AppData/Roaming/kicad/*/*.json"):
    try: c = json.load(open(f))
    except Exception: continue
    if isinstance(c, dict) and "aui" in c: c.pop("aui"); json.dump(c, open(f, "w"), indent=2)
PY
start_xvfb_and_wm; start_hub
echo "{\"viewer_px\": $SW}" > "$OUT/local-settings.json"
cat > "$OUT/script.json" <<'J'
[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"up"}},
 {"wait":6000},{"kicad":{"report":true,"tag":"pcb"}},{"mark":"pcb"},{"wait":3000},
 {"click":"kicad-tab:SCH"},{"wait":9000},{"kicad":{"report":true,"tag":"sch"}},{"mark":"sch"},{"wait":3000},
 {"click":"kicad-tab:PCB"},{"wait":4000},{"click":"kicad:full"},{"wait":6000},{"kicad":{"report":true,"tag":"full"}},{"mark":"full"},{"wait":5000},
 {"click":"kicad:full"},{"wait":5000},{"click":"kicad:full"},{"wait":6000},{"kicad":{"report":true,"tag":"reopen"}},{"mark":"reopen"},{"wait":30000}]
J
HOME="$OUT/fh" BACKPLANE_GPUI_LOCAL="$OUT/local-settings.json" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mkdir -p "$OUT/shots"
# the frame on show at a mark: its own picture, the pane's width, and the canvas in it
measure() {
  local tag=$1 R xid
  wait_for_log "$OUT/client.log" "\"name\":\"$tag\",\"event\":\"mark\"" 1200 || { echo "0 0 0 0 no-$tag"; return; }
  R=$(last_kicad_report "$OUT/client.log" "$tag")
  xid=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("shown") or 0)' "$R")
  import -window "$xid" "$OUT/shots/$tag.png" 2>/dev/null || true
  python3 - "$OUT/shots/$tag.png" "$R" <<'PY'
import json, sys
sys.path.insert(0, __import__("os").environ["KEMBED_LIB"])
import numpy as np
from PIL import Image
from canvas_box import canvas_box
r = json.loads(sys.argv[2] or "{}"); pane = (r.get("pane") or {}).get("w", 0)
try: a = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
except Exception: print(pane, 0, 0, 0, "no-picture"); sys.exit()
b = canvas_box(a)
# (canvas_box answers the whole frame when it finds no canvas: that is no measure)
if b == (0, 0, a.shape[1] - 1, a.shape[0] - 1): print(pane, a.shape[1], 0, 0, "no-canvas-found"); sys.exit()
cw = b[2] - b[0]
# the columns beside the canvas: left (KiCad's Properties, with the left tool bar) and right (Appearance, with the right tool bar)
print(pane, a.shape[1], cw, round(cw / pane, 2) if pane else 0, "ok", b[0], a.shape[1] - b[2])
PY
}
export KEMBED_LIB
# a runtime without autopanels (before bp.13): KiCad's defaults show the side panels at
# any width (the canvas a sliver in a narrow pane: bp.12.4 0.44 at 611 px, Windows bp.10): the canvas share is said, not checked; the
# fix for people is the bp.13 runtime, which the hub updates to
AUTOP=$(python3 -c 'import re,sys; m=re.search(r"bp\.(\d+)", sys.argv[1]); print(int(bool(m) and int(m.group(1)) >= 13))' "$(grep -o '"version": *"[^"]*"' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null | head -1)")
read -r P1 F1 C1 R1 S1 L1 G1 <<<"$(measure pcb)"
echo "INFO $SW px pane, board editor: pane $P1, frame $F1, canvas $C1 ($R1 of the pane; $S1; left of it ${L1:-?} px, right ${G1:-?} px)"
if [ "$AUTOP" = 1 ]; then check "$SW px pane: the board canvas is at least 60 % of the pane's width (fresh profile, after Edit)" "$(python3 -c "print(int($R1 >= 0.6))")" "pane $P1, canvas $C1"
else echo "INFO $SW px pane: a runtime without autopanels: the board canvas $R1 of the pane (not checked)"; fi
read -r P2 F2 C2 R2 S2 L2 G2 <<<"$(measure sch)"
echo "INFO $SW px pane, schematic editor: pane $P2, frame $F2, canvas $C2 ($R2 of the pane; $S2)"
if [ "$AUTOP" = 1 ]; then check "$SW px pane: the schematic canvas is at least 60 % of the pane's width" "$(python3 -c "print(int($R2 >= 0.6))")" "pane $P2, canvas $C2"
else echo "INFO $SW px pane: a runtime without autopanels: the schematic canvas $R2 of the pane (not checked)"; fi
read -r P3 F3 C3 R3 S3 L3 G3 <<<"$(measure full)"
echo "INFO $SW px pane at Full size, board editor: pane $P3, frame $F3, canvas $C3 ($R3; $S3; left ${L3:-?} px, right ${G3:-?} px)"
[ "$AUTOP" = 1 ] || echo "INFO $SW px pane: a runtime without autopanels: Full size and a closed panel not checked"
[ "$AUTOP" = 1 ] && check "$SW px pane, Full size: Appearance and Properties visible on a fresh profile (the frame wider than the canvas by at least 150 px)" "$([ "$P3" -gt "$P1" ] && [ $((F3 - C3)) -ge 150 ] && echo 1 || echo 0)" "pane $P1 -> $P3, frame $F3, canvas $C3"
# a panel the person closed stays closed: Appearance closed at Full size (View > Appearance, KiCad's own action), Full size off (the pane
# narrow) and on again: Properties comes back, Appearance does not (the right column only the tool bar)
if [ "$AUTOP" = 1 ] && [ "${S3:-}" = ok ] && [ "${G3:-0}" -ge 150 ]; then
  SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock") or "")' "$(last_kicad_report "$OUT/client.log" full)")
  python3 "$IPC_PY" run-action "$SOCK" "pcbnew.Control.showLayersManager" >/dev/null 2>&1 || true
  read -r P4 F4 C4 R4 S4 L4 G4 <<<"$(measure reopen)"
  echo "INFO $SW px pane, Appearance closed, Full size off and on: frame $F4, canvas $C4 (left ${L4:-?} px, right ${G4:-?} px)"
  check "$SW px pane: Appearance closed by the person stays closed after narrow and wide again (Properties back)" "$([ "${G4:-999}" -lt 100 ] && [ "${L4:-0}" -ge 150 ] && echo 1 || echo 0)" "left ${L4:-?}, right ${G4:-?}"
fi
kill "$CP" 2>/dev/null || true
finish_suite "embed_narrow_pane_e2e ($EMBED_MODE, $SW px)"
