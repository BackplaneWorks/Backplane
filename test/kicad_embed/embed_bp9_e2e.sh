#!/bin/bash
# Suite 6 (fork mode, Backplane KiCad bp.9 or later: its api.desc has SetTheme and ReloadDocument; skipped otherwise):
#  - the board written on disk by someone else (an atomic rename, as an agent's script does) reloads in the open editor by itself;
#  - written again over unsaved edits: nothing is reloaded, the hub says so (info kicad.reload.<digest> -> the pane's conflicts), and
#    "Keep yours" (editor.kicad.resolve) keeps the edits;
#  - KiCad follows Backplane's theme live: light, then dark, on the running process (the canvas's brightness).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_bp9_e2e: fork only"; exit 0; }
discover_bins
discover_kicad
DESC="$(dirname "$KICAD_BIN")/../share/backplane/api.desc"
if ! grep -qa "SetTheme" "$DESC" 2>/dev/null || ! grep -qa "ReloadDocument" "$DESC" 2>/dev/null; then echo "embed_bp9_e2e: SKIP (this KiCad has no SetTheme/ReloadDocument: needs bp.9)"; exit 0; fi
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
PCB="$OUT/proj/ecc83-pp.kicad_pcb"
# the client: Edit, then marks the shell acts on (the waits leave it the time)
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 15000}]
for m, w in [("ready", 9000), ("reloaded", 9000), ("dirty", 9000), ("conflict", 9000), ("kept", 6000), ("light", 6000), ("dark", 4000)]:
    s += [{"mark": m}, {"kicad": {"report": True}}, {"wait": w}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 130 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_mark() { for _ in $(seq 600); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }
rep() { last_kicad_report "$OUT/client.log" "$1"; }
field() { python3 -c 'import json,sys; r=json.loads(sys.argv[1]); v=r.get(sys.argv[2]); print(v if v is not None else "")' "$1" "$2"; }
tracks() { python3 "$IPC_PY" items-count "$SOCK" pcb 11 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])' 2>/dev/null || echo -1; }
# a track written into the board file by someone else: a temp file renamed over it (atomic), as scripts and kicad-cli do
write_track() { python3 - "$PCB" "$1" <<'PY'
import os, sys, uuid
p, y = sys.argv[1], float(sys.argv[2]); s = open(p).read().rstrip()
assert s.endswith(")")
seg = f'\n\t(segment (start 120 {y}) (end 140 {y}) (width 0.25) (layer "F.Cu") (net 0) (uuid "{uuid.uuid4()}"))\n)'
open(p + ".tmp", "w").write(s[:-1] + seg + "\n"); os.replace(p + ".tmp", p)
PY
}
canvas_light() { # mean brightness (0..255) of the shown frame's middle, from the screen
  python3 - "$1" <<'PY'
import json, subprocess, sys
r = json.loads(sys.argv[1]); f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown")), {})
x, y, w, h = f.get("root_x", 0) + f.get("w", 0) // 3, f.get("root_y", 0) + f.get("h", 0) // 3, max(1, f.get("w", 0) // 3), max(1, f.get("h", 0) // 3)
out = subprocess.run(["import", "-window", "root", "-crop", f"{w}x{h}+{x}+{y}", "-colorspace", "Gray", "-format", "%[fx:int(255*mean)]", "info:"], capture_output=True, text=True).stdout.strip()
print(out or -1)
PY
}
wait_mark ready || true; sleep 1; R=$(rep ready); SOCK=$(field "$R" sock)
N0=$(tracks); write_track 60; 
wait_mark reloaded || true; sleep 1; N1=$(tracks)
check "a board written on disk (atomic rename) reloads in the open editor by itself" "$([ "$N0" -ge 0 ] && [ "$N1" -eq $((N0 + 1)) ] && echo 1 || echo 0)" "$N0 -> $N1"
shot reloaded
python3 "$IPC_PY" agent-add-track "$SOCK" 120 70 140 70 0.25 >/dev/null 2>&1 || true   # unsaved edits in the editor (an agent's, uncommitted to disk)
wait_mark dirty || true; N2=$(tracks); write_track 80
wait_mark conflict || true; sleep 1; R=$(rep conflict); N3=$(tracks)
CONF=$(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1]).get("conflicts") or []))' "$R")
check "written again over unsaved edits: nothing reloaded, the pane says it waits on the person" "$([ "$N3" -eq "$N2" ] && [ "$CONF" -ge 1 ] && echo 1 || echo 0)" "tracks $N2 -> $N3, conflicts=$CONF"
shot conflict
RES=$(hub_rpc "[{\"m\":\"editor.kicad.resolve\",\"p\":{\"project\":\"$PCB\",\"file\":\"$PCB\",\"choice\":\"keep\"}}]" || true); echo "$RES" >"$OUT/resolve.log"
wait_mark kept || true; sleep 1; R=$(rep kept); N4=$(tracks)
CONF=$(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1]).get("conflicts") or []))' "$R")
check "Keep yours: the edits stay, and nothing waits any more" "$([ "$N4" -eq "$N2" ] && [ "$CONF" -eq 0 ] && echo 1 || echo 0)" "tracks $N4 (edits $N2), conflicts=$CONF, $RES"
hub_rpc "[{\"m\":\"editor.kicad.theme\",\"p\":{\"project\":\"$PCB\",\"mode\":\"light\"}}]" >"$OUT/theme_light.log" 2>&1 || true
wait_mark light || true; sleep 1.5; L=$(canvas_light "$(rep light)"); shot light
hub_rpc "[{\"m\":\"editor.kicad.theme\",\"p\":{\"project\":\"$PCB\",\"mode\":\"dark\"}}]" >"$OUT/theme_dark.log" 2>&1 || true
wait_mark dark || true; sleep 2; D=$(canvas_light "$(rep dark)"); shot dark
check "KiCad follows Backplane's theme live: light, then dark, on the running editor" "$([ "$L" -gt 150 ] && [ "$D" -lt 90 ] && echo 1 || echo 0)" "light=$L dark=$D $(cat "$OUT/theme_light.log" | head -c 200)"
wait "$CP" 2>/dev/null || true
finish_suite "embed_bp9_e2e ($EMBED_MODE)"
