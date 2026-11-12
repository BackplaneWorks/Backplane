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
# the client: Edit, waits until the board editor is painted (KiCad's state, however slow the machine), then reports the pane every second;
# the shell acts and polls for each outcome (tracks over the API, the pane's conflicts, the canvas's brightness) up to a ceiling, and how
# long each took is a separate timing check (KEMBED_TIME_SCALE multiplies the budgets on a slow venue)
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"},
     {"kicad": {"wait_painted": 150000, "kind": "PCB", "tag": "ready"}}, {"mark": "ready"}]
for i in range(240): s += [{"wait": 1000}, {"kicad": {"report": True, "tag": "tick"}}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 600 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_mark() { for _ in $(seq 600); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }
rep() { last_kicad_report "$OUT/client.log" "$1"; }
field() { python3 -c 'import json,sys; r=json.loads(sys.argv[1]); v=r.get(sys.argv[2]); print(v if v is not None else "")' "$1" "$2"; }
tracks() { python3 "$IPC_PY" items-count "$SOCK" pcb 11 2>>"$OUT/ipc.err" | python3 -c 'import json,sys; print(json.load(sys.stdin)["count"])' 2>>"$OUT/ipc.err" || echo -1; }
# a track written into the board file by someone else: a temp file renamed over it (atomic), as scripts and kicad-cli do
write_track() { python3 - "$PCB" "$1" <<'PY'
import os, sys, uuid
p, y = sys.argv[1], float(sys.argv[2]); s = open(p).read().rstrip()
assert s.endswith(")")
seg = f'\n\t(segment (start 120 {y}) (end 140 {y}) (width 0.25) (layer "F.Cu") (net 0) (uuid "{uuid.uuid4()}"))\n)'
open(p + ".tmp", "w").write(s[:-1] + seg + "\n"); os.replace(p + ".tmp", p)
PY
}
canvas_light() { # mean brightness (0..255) of the shown frame's menu and toolbar row, from the screen
  python3 - "$1" <<'PY'
import json, subprocess, sys
r = json.loads(sys.argv[1]); f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown")), {})
# the frame's own toolbars (GTK widgets): white-ish in light, near black in dark (the PCB canvas of KiCad's default colours is dark in both)
x, y, w, h = f.get("root_x", 0) + 4, f.get("root_y", 0) + 2, max(1, f.get("w", 0) - 8), 24
png = subprocess.run(["import", "-window", "root", "png:-"], capture_output=True).stdout
out = subprocess.run(["convert", "png:-", "-crop", f"{w}x{h}+{x}+{y}", "+repage", "-colorspace", "Gray", "-format", "%[fx:int(255*mean)]", "info:"], input=png, capture_output=True).stdout.decode().strip()
print(out or -1)
PY
}
SCALE=${KEMBED_TIME_SCALE:-1}
now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
# poll CMD until its output satisfies TEST (a python expression on v), at most SECS: prints the last value; $WAITED_MS is how long it took
poll() { local secs=$1 test=$2; shift 2; local t0 v; t0=$(now_ms)
  while :; do v=$("$@" 2>/dev/null || echo -1); python3 -c "import sys; v=sys.argv[1]; sys.exit(0 if ($test) else 1)" "$v" && break
    [ $(( $(now_ms) - t0 )) -ge $((secs * 1000)) ] && break; sleep 0.25; done
  WAITED_MS=$(( $(now_ms) - t0 )); echo "$v"; }
timing() { echo "PERF bp9_$1 ${2}ms"; check "time: $3 within $(python3 -c "print(int($4*$SCALE))") ms" "$(python3 -c "print(int(0 <= $2 <= $4*$SCALE))")" "$2 ms"; }
conflicts() { python3 -c 'import json,sys; print(len(json.loads(sys.argv[1] or "{}").get("conflicts") or []))' "$(rep tick)"; }
wait_mark ready || true; R=$(rep ready); SOCK=$(field "$R" sock)
timing edit "$(field "$R" wait_ms)" "Edit to the Board painted" 15000
# the fixture's board is from an older KiCad, so it opens modified (converted on save): saved first, it is clean
python3 "$IPC_PY" save-doc "$SOCK" pcb >"$OUT/save.log" 2>&1 || true
poll 20 'v == "0"' is_doc_modified "$SOCK" pcb 0 >/dev/null
N0=$(tracks); write_track 60
N1=$(poll 60 "int(v) == $N0 + 1" tracks); RELOAD_MS=$WAITED_MS
check "a board written on disk (atomic rename) reloads in the open editor by itself" "$([ "$N0" -ge 0 ] && [ "$N1" -eq $((N0 + 1)) ] && echo 1 || echo 0)" "$N0 -> $N1"
timing reload "$RELOAD_MS" "the write on disk reloaded" 5000
shot reloaded
python3 "$IPC_PY" agent-add-track "$SOCK" 120 70 140 70 0.25 >/dev/null 2>&1 || true   # unsaved edits in the editor (an agent's, uncommitted to disk)
N2=$(poll 20 "int(v) == $N1 + 1" tracks); write_track 80
C=$(poll 60 'int(v) >= 1' conflicts); CONF_MS=$WAITED_MS; N3=$(tracks)
check "written again over unsaved edits: nothing reloaded, the pane says it waits on the person" "$([ "$N3" -eq "$N2" ] && [ "$C" -ge 1 ] && echo 1 || echo 0)" "tracks $N2 -> $N3, conflicts=$C"
timing conflict "$CONF_MS" "the conflict shown in the pane" 5000
shot conflict
RES=$(hub_rpc "[{\"m\":\"editor.kicad.resolve\",\"p\":{\"project\":\"$PCB\",\"file\":\"$PCB\",\"choice\":\"keep\"}}]" || true); echo "$RES" >"$OUT/resolve.log"
C=$(poll 30 'int(v) == 0' conflicts); N4=$(tracks)
check "Keep yours: the edits stay, and nothing waits any more" "$([ "$N4" -eq "$N2" ] && [ "$C" -eq 0 ] && echo 1 || echo 0)" "tracks $N4 (edits $N2), conflicts=$C, $RES"
hub_rpc "[{\"m\":\"editor.kicad.theme\",\"p\":{\"project\":\"$PCB\",\"mode\":\"light\"}}]" >"$OUT/theme_light.log" 2>&1 || true
light_now() { canvas_light "$(rep tick)"; }
L=$(poll 30 'int(v) > 150' light_now); LIGHT_MS=$WAITED_MS; shot light
hub_rpc "[{\"m\":\"editor.kicad.theme\",\"p\":{\"project\":\"$PCB\",\"mode\":\"dark\"}}]" >"$OUT/theme_dark.log" 2>&1 || true
D=$(poll 30 '0 <= int(v) < 90' light_now); DARK_MS=$WAITED_MS; shot dark
check "KiCad follows Backplane's theme live: light, then dark, on the running editor" "$([ "$L" -gt 150 ] && [ "$D" -ge 0 ] && [ "$D" -lt 90 ] && echo 1 || echo 0)" "light=$L dark=$D $(cat "$OUT/theme_light.log" | head -c 200)"
timing theme "$(( LIGHT_MS > DARK_MS ? LIGHT_MS : DARK_MS ))" "a theme change on screen" 5000
stop_pid "$CP"
wait "$CP" 2>/dev/null || true
finish_suite "embed_bp9_e2e ($EMBED_MODE)"
