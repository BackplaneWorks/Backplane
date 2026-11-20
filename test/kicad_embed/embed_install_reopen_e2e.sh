#!/bin/bash
# A fresh home with no KiCad: the person's Edit is refused ("KiCad was not found"), they install KiCad (the consented kicad.install, as
# Settings' Install sends it), and when the install is done the Edit they asked for comes up in the pane by itself: KiCad attached to the
# pane's container and drawn there, never started in a window of its own beside Backplane (bp.13.3 release, Windows fresh3/fresh4: the
# update row's reopen sent editor.kicad.restart with no container, the hub started KiCad in its own window, and the pane asked for a show
# every 2 s for 21 minutes on "Starting KiCad...").
#   I1  the Edit on a fresh home is refused: KiCad was not found
#   I2  the install the person consents to completes (a stand-in installer puts the given runtime where the hub looks)
#   I3  within 60 s of the install's end the pane shows the board, drawn, in its container
#   I4  no KiCad window on screen outside the pane (a top-level of KiCad's that is not inside Backplane's window)
#   I5  the pane is not left asking for shows: at most 3 editor.kicad.show in the last 20 s
# NOKICAD_RUNTIME=<a Backplane KiCad's bin/kicad>: the runtime the stand-in installs (remote.sh ships it). Fork mode, X11.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_install_reopen_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
RT_ROOT=$(cd "$(dirname "${NOKICAD_RUNTIME:?NOKICAD_RUNTIME=<runtime>/bin/kicad}")/.." && pwd)
export BACKPLANE_KICAD_NO_SYSTEM=1 BACKPLANE_KICAD_DEBUG=1 KICAD_CLI= BACKPLANE_KICAD_CLI=
# the stand-in installer: the installer's arguments (parent, platform, version, ...), the given runtime linked in as <parent>/<version>
ST="$OUT/bin/standin-installer"
printf '#!/bin/sh\necho "Downloading Backplane KiCad (stand-in)"\nsleep 2\nmkdir -p "$1" && ln -sfn "%s" "$1/$3" && echo "Installed (stand-in)"\n' "$RT_ROOT" > "$ST"; chmod +x "$ST"
export BACKPLANE_KICAD_INSTALLER="$ST"
start_xvfb_and_wm; start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2500}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 5000}, {"kicad": {"report": True, "tag": "refused"}}, {"mark": "refused"},
     {"wait": 75000}, {"kicad": {"report": True, "tag": "after"}}, {"mark": "after"}, {"wait": 2000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 140 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mark() { for _ in $(seq 1200); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }
mark refused || true; shot refused
REFUSED=$(grep -c '"kicad_rpc_reply":"editor.kicad.attach".*KiCad was not found' "$OUT/client.log" || true)
check "I1 the Edit on a fresh home is refused: KiCad was not found" "$([ "${REFUSED:-0}" -ge 1 ] && echo 1 || echo 0)" "refusals $REFUSED"
# the person's consented install
hub_rpc '[{"method":"kicad.install","timeout_ms":20000,"params":{"consent":true}}]' > "$OUT/install.out" 2>&1 || true
DONE=0; for _ in $(seq 120); do grep -q "kicad: installed Backplane KiCad" "$OUT/hub.log" && { DONE=1; break; }; sleep 0.5; done
T_DONE=$(date +%s)
check "I2 the consented install completes" "$DONE" "$(grep -o 'kicad: install.*' "$OUT/hub.log" | tail -1)"
# the pane's board, drawn, in its container (polled from the client's own reports is not possible here: the hub's session says the container)
ATT=0; for _ in $(seq 120); do
  R=$(hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- || true)
  read -r ST CONT < <(python3 -c 'import json,sys
try: v=json.loads(sys.argv[1]).get("value") or {}
except Exception: v={}
print(v.get("state","-"), v.get("container",0))' "$R" 2>/dev/null || echo "- 0")
  [ "$ST" = ready ] && [ "${CONT:-0}" != 0 ] && { ATT=1; break; }; sleep 0.5; done
echo "INFO after the install: session state $ST, container $CONT, $(( $(date +%s) - T_DONE )) s after done"
mark after || true; shot after
R=$(last_kicad_report "$OUT/client.log" after)
PAINT=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(f.get("kind","-"), f.get("painted",0), r.get("say") or "")' "$R")
echo "INFO the pane after: $PAINT"
check "I3 the pane shows the board, drawn, attached to its container" "$([ "$ATT" = 1 ] && python3 -c 'import sys; a=sys.argv[1].split(); sys.exit(0 if a[0]=="PCB" and int(a[1])>=15 else 1)' "$PAINT" && echo 1 || echo 0)" "session $ST container $CONT; pane $PAINT"
# a KiCad top-level outside Backplane's window
STRAY=$(python3 - "$KEMBED_LIB" <<'PY'
import sys
sys.path.insert(0, sys.argv[1]); import x11_embed_driver as d
lib = d.load_lib(); out = []
for w in d.list_all_windows(lib):
    t = (w["title"] + " " + w["wm_class"]).lower()
    if w["shown"] and not w["override_redirect"] and w["w"] >= 100 and ("kicad" in t or "pcb editor" in t) and "backplane" not in t: out.append(f'{w["xid"]}:{w["title"][:40]}@{w["root_x"]},{w["root_y"]}')
print(" ".join(out) or "none")
PY
)
echo "INFO KiCad windows outside the pane: $STRAY"
check "I4 no KiCad window on screen outside the pane" "$([ "$STRAY" = none ] && echo 1 || echo 0)" "$STRAY"
SHOWS=$(python3 - "$OUT/client.log" <<'PY'
import json, sys
ts = []; end = 0
for l in open(sys.argv[1], errors="ignore"):
    if '"kicad_rpc_sent":"editor.kicad.show"' in l:
        try: ts.append(json.loads(l.split("GPUI_LOG ", 1)[1])["at_ms"])
        except Exception: pass
    if '"name":"after","event":"mark"' in l:
        try: end = json.loads(l.split("GPUI_METRIC ", 1)[1])["epoch_ms"]
        except Exception: pass
print(len(ts), sum(1 for t in ts if end and 0 <= end - t <= 20000))
PY
)
echo "INFO editor.kicad.show sent: total, in the last 20 s: $SHOWS"
check "I5 the pane is not left asking for shows (at most 3 in the last 20 s)" "$(python3 -c 'import sys; sys.exit(0 if int(sys.argv[2]) <= 3 else 1)' $SHOWS && echo 1 || echo 0)" "$SHOWS"
wait "$CP" 2>/dev/null || true
finish_suite "embed_install_reopen_e2e ($EMBED_MODE)"
