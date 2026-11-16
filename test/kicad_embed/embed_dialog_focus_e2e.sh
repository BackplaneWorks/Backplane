#!/bin/bash
# KiCad's own dialogs and the keyboard (lib/dialog_focus.py): after Backplane's palette and comment window (what left real_use's dialog
# without the keyboard, 2 of 5 runs on i3), a dialog KiCad brings up by a key, by a toolbar click or from inside a dialog has the keyboard,
# Escape closes it and the keyboard is the canvas's again; a dialog that comes up while the person types in Backplane leaves the keyboard
# where it is. The client gives a new KiCad dialog the keyboard only when it was KiCad's (kicad_pane.rs, native_child.rs activate). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_dialog_focus_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
cat > "$OUT/script.json" <<'J'
[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"pcb"}},{"wait":4000},{"kicad":{"report":true,"tag":"r"}},{"mark":"ready"},{"wait":240000},{"mark":"done"}]
J
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 1200); do grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
export KEMBED_LIB DRIVER_PY IPC_PY REPO_ROOT PCB; PCB=$(ls "$OUT"/proj/*.kicad_pcb | head -1)
python3 "$KEMBED_LIB/dialog_focus.py" "$OUT" > "$OUT/drive.log" 2>&1 || true
grep '^INFO' "$OUT/drive.log" || true
while read -r _ ok what; do check "$what" "$ok"; done < <(grep '^CHECK ' "$OUT/drive.log")
grep -q '^CHECK ' "$OUT/drive.log" || { tail -20 "$OUT/drive.log"; check "the drive ran" 0; }
grep -c '"kicad_activate"' "$OUT/client.log" | sed 's/^/INFO dialogs the client activated: /' || true
kill "$CP" 2>/dev/null || true
finish_suite "embed_dialog_focus_e2e ($EMBED_MODE)"
