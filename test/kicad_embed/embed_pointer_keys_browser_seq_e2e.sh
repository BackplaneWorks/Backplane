#!/bin/bash
# The Browser tab's page and a KiCad pane in one client, through the sequences a person can reach today (one viewer page shows at a time:
# a KiCad tile beside the Browser in the same window is not reachable, so not tested here). Backplane holds the keyboard unless the person
# clicked a child; keys typed with the pointer over a child that was not clicked must land in the composer. Needs a client built with the
# embedded Chromium (feature cef) and the fork KiCad (EMBED_MODE=fork). A page logs every keydown into its title (`k:<keys>`).
#   S1 the Browser shown, the pane alive but hidden: a click in the page gives the page the keys, and the pane says keys=false
#   S2 Browser -> board pane -> Browser: over the pane (no click) the keys land in the composer; a click in KiCad gives KiCad the keys;
#      Ctrl+` gives them back (keys=false); back on the Browser, over the page (no click) the composer, a click in the page the page
#   S3 the pane popped out into its own window, the Browser in the main one: over the page (no click) the composer, a click in the page the
#      page (keys=false); the mirror: a click in the popped-out KiCad (keys=true), then a click in the main window's page gives the page the
#      keys and the pane says keys=false
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_pointer_keys_browser_seq_e2e: fork only (EMBED_MODE=fork)"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
SHOTS="$OUT/barriers"; mkdir -p "$SHOTS"
PAGE='data:text/html,<body style="margin:0;background:%23e8f0ff"><h1>keys</h1><input id=t><script>let k="";document.addEventListener("keydown",e=>{k+=e.key;document.title="k:"+k})</script></body>'
PAGE_JSON=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$PAGE")
# each phase: the client reports, marks, and waits at a barrier (a "shot" whose .done the driver below writes) while the driver types
REP='{"read":"composer"},{"cef":{"report":true}},{"kicad":{"report":true}}'
bar() { printf '{"mark":"%s"},{"shot":"%s","timeout_ms":180000}' "$1" "$1"; }
cat >"$OUT/script.json" <<JSON
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":3000},
 {"click":"rail:Browser"},{"wait":5000},{"cef":{"goto":$PAGE_JSON}},{"wait":4000},$REP,$(bar s1),$REP,{"mark":"r1"},
 {"click":"rail:Board"},{"wait":3000},$REP,$(bar s2a),$REP,$(bar s2b),{"wait":800},$REP,{"mark":"k2c"},
 {"click":"rail:Browser"},{"wait":3000},$REP,$(bar s2d),$REP,{"mark":"r2"},
 {"click":"rail:Board"},{"wait":2000},{"click":"kicad:popout"},{"wait":5000},{"click":"rail:Browser"},{"wait":3000},$REP,$(bar s3),$REP,{"mark":"r3"},
 $(bar s3m),{"wait":800},$REP,{"mark":"k3m"},$(bar s3n),$REP,{"mark":"done"},{"wait":600000}]
JSON
HOME="$OUT/fh" LC_ALL=C.UTF-8 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_SHOT_DIR="$SHOTS" BACKPLANE_CEF_PROFILE="$OUT/prof" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" \
  --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 900 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
TOP=
top_xy() { python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid"]==sys.argv[1]]; print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)' "$TOP"; }
# what the client said last before mark $1: composer text, the page's title, the pane's keys, the page's middle and the pane's middle (root)
state() { python3 - "$OUT/client.log" "$1" "$TX" "$TY" <<'PY'
import json, sys
log, mark, tx, ty = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
t = title = keys = r = k = c = None
for l in open(log, errors="ignore"):
    if "GPUI_METRIC " not in l: continue
    try: o = json.loads(l.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if o.get("event") == "mark" and o.get("name") == mark: break
    if isinstance(o.get("rects"), dict) and "composer" in o["rects"]: c = o["rects"]["composer"]
    if isinstance(o.get("rects"), list):
        for x in o["rects"]:
            if x.get("id") == "composer": c = x
    if o.get("event") == "read" and o.get("what") == "composer": t = o.get("text")
    if o.get("event") == "cef" and "title" in o: title = o.get("title")
    if o.get("event") == "cef" and o.get("view_x") is not None: r = o
    if o.get("event") == "kicad" and "keys" in o: keys = o.get("keys"); k = o.get("container")
gx = tx + int(r["view_x"] + r["frame"][0] / 2) if r else 0; gy = ty + int(r["view_y"] + r["frame"][1] / 2) if r else 0
cx = tx + int(c["x"] + c["w"] / 2) if c else 0; cy = ty + int(c["y"] + c["h"] / 3) if c else 0
kx = int(k["root_x"] + k["w"] / 2) if k and k.get("root_x") is not None else 0; ky = int(k["root_y"] + k["h"] / 2) if k and k.get("root_y") is not None else 0
print("\t".join([json.dumps(t or ""), json.dumps(title or ""), json.dumps(keys)] + [str(v) for v in (gx, gy, cx, cy, kx, ky)]))
PY
}
release() { wait_for_log "$OUT/client.log" "\"name\":\"$1\",\"event\":\"mark\"" 900 || { echo "FAIL the client never reached $1"; finish_suite "embed_pointer_keys_browser_seq_e2e"; exit 1; }; sleep 0.5; }
go_on() { touch "$SHOTS/$1.done"; }
type_word() { local w=$1 i; for ((i = 0; i < ${#w}; i++)); do python3 "$XINPUT_PY" key "${w:i:1}" >/dev/null; sleep 0.15; done; }
click() { python3 "$XINPUT_PY" click "$1" "$2" >/dev/null; sleep 1.0; }
move() { python3 "$XINPUT_PY" move "$1" "$2" >/dev/null; sleep 0.8; }
say() { echo "INFO $1: focus [$(xfocus_chain)]"; }
has() { python3 -c 'import json,sys; print(1 if sys.argv[2] in (json.loads(sys.argv[1]) or "") else 0)' "$1" "$2"; }
logged() { python3 -c 'import json,sys; t=json.loads(sys.argv[1]) or ""; print(1 if t.startswith("k:") and sys.argv[2] in t[2:] else 0)' "$1" "$2"; }
not_logged() { python3 -c 'import json,sys; t=json.loads(sys.argv[1]) or ""; print(0 if t.startswith("k:") and sys.argv[2] in t[2:] else 1)' "$1" "$2"; }
is() { [ "$1" = "$2" ] && echo 1 || echo 0; }

release s1
TOP=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if "Backplane GPUI" in (w.get("title") or "")]; print(w[0]["xid"] if w else "")')
read -r TX TY < <(top_xy)
IFS=$'\t' read -r _ _ _ GX GY CX CY _ _ < <(state s1)
echo "INFO top $TOP; page root ($GX, $GY); composer root ($CX, $CY)"
if [ "${GX:-0}" -le 0 ] || [ "${CX:-0}" -le 0 ]; then check "S0 the page's and the composer's places are known" 0 "page ($GX, $GY) composer ($CX, $CY)"; finish_suite "embed_pointer_keys_browser_seq_e2e"; exit 1; fi
# S1: the Browser, the pane alive but hidden
click "$CX" "$CY"
# the window manager's part (a click-to-focus putting the X focus on the top-level itself), played GAPS times: how long until it is on the proxy
python3 "$HERE/lib/focus_trace.py" gap "$TOP" "${GAPS:-20}" >"$OUT/gap-s1.log" 2>&1; G1=$(tail -1 "$OUT/gap-s1.log"); echo "INFO S1 gap (the Browser on screen, the pane hidden): $G1"
move "$GX" "$GY"; type_word ha; say "S1 over the page, no click"
click "$GX" "$GY"; type_word hp; say "S1 a click in the page"
go_on s1; release r1
IFS=$'\t' read -r T1 P1 K1 _ < <(state r1)
check "S1a the pane hidden, over the page (no click): the composer" "$(has "$T1" ha)" "composer $T1, page $P1"
check "S1b a click in the page: the page has the keys" "$(logged "$P1" hp)" "page $P1"
check "S1c and the pane says keys=false" "$(is "$K1" false)" "keys $K1"
all_moved() { [[ $1 =~ moved\ ([0-9]+)/([0-9]+) ]] && [ "${BASH_REMATCH[1]}" = "${BASH_REMATCH[2]}" ] && [ "${BASH_REMATCH[2]}" -gt 0 ] && echo 1 || echo 0; }
check "S1d the Browser on screen: a focus put on the top-level moves to its proxy, every time" "$(all_moved "$G1")" "$G1"
# S2: the board pane shown
release s2a
IFS=$'\t' read -r _ _ _ _ _ _ _ KX KY < <(state s2a)
echo "INFO S2 pane root ($KX, $KY)"
click "$CX" "$CY"
python3 "$HERE/lib/focus_trace.py" gap "$TOP" "${GAPS:-20}" >"$OUT/gap-s2.log" 2>&1; G2=$(tail -1 "$OUT/gap-s2.log"); echo "INFO S2 gap (the pane on screen): $G2"
move "$KX" "$KY"; type_word pb; say "S2 over the pane, no click"
click "$KX" "$KY"; say "S2 a click in KiCad"
go_on s2a; release s2b
IFS=$'\t' read -r T2 _ K2 _ < <(state s2b)
check "S2a over the pane (no click): the composer" "$(has "$T2" pb)" "composer $T2"
check "S2g the pane on screen: a focus put on the top-level moves to its proxy, every time" "$(all_moved "$G2")" "$G2"
check "S2b a click in KiCad hands it the keys" "$(is "$K2" true)" "keys $K2"
python3 "$XINPUT_PY" chord ctrl grave >/dev/null; sleep 0.5; say "S2 Ctrl+\` in KiCad"
go_on s2b; release k2c
IFS=$'\t' read -r _ _ K2C _ < <(state k2c)
check "S2c Ctrl+\` gives the keys back (keys=false)" "$(is "$K2C" false)" "keys $K2C"
release s2d
click "$CX" "$CY"; move "$GX" "$GY"; type_word rb; say "S2 back on the Browser, over the page"
click "$GX" "$GY"; type_word rp; say "S2 a click in the page"
go_on s2d; release r2
IFS=$'\t' read -r T2D P2D K2D _ < <(state r2)
check "S2d back on the Browser, over the page (no click): the composer" "$(has "$T2D" rb)" "composer $T2D, page $P2D"
check "S2e a click in the page: the page" "$(logged "$P2D" rp)" "page $P2D"
check "S2f and the pane says keys=false" "$(is "$K2D" false)" "keys $K2D"
# S3: the pane popped out, the Browser in the main window
release s3
# (the window manager may have moved and resized the main window to make room for the popped-out one: everything measured again)
read -r TX TY < <(top_xy)
IFS=$'\t' read -r _ _ _ GX GY CX CY PX PY < <(state s3)
echo "INFO S3 main window at ($TX, $TY): page root ($GX, $GY), composer root ($CX, $CY); popped-out pane root ($PX, $PY)"
click "$CX" "$CY"; move "$GX" "$GY"; type_word ob; say "S3 over the page, pane popped out"
click "$GX" "$GY"; type_word op; say "S3 a click in the page"
go_on s3; release r3
IFS=$'\t' read -r T3 P3 K3 _ < <(state r3)
check "S3a pane popped out, over the page (no click): the composer" "$(has "$T3" ob)" "composer $T3, page $P3"
check "S3b a click in the page: the page" "$(logged "$P3" op)" "page $P3"
check "S3c and the pane says keys=false" "$(is "$K3" false)" "keys $K3"
release s3m
if [ "${PX:-0}" -gt 0 ]; then click "$PX" "$PY"; say "S3 a click in the popped-out KiCad"; fi
go_on s3m; release k3m
IFS=$'\t' read -r _ _ K3M _ < <(state k3m)
check "S3d the mirror: a click in the popped-out KiCad gives it the keys" "$(is "$K3M" true)" "keys $K3M, pane ($PX, $PY)"
release s3n
click "$GX" "$GY"; type_word mp; say "S3 a click in the main window's page"
go_on s3n; release done
IFS=$'\t' read -r T3N P3N K3N _ < <(state done)
check "S3e then a click in the main window's page: the page" "$(logged "$P3N" mp)" "page $P3N"
check "S3f and the pane says keys=false" "$(is "$K3N" false)" "keys $K3N"
check "S3g nothing typed in KiCad reached the composer or the page by itself" "$(not_logged "$P3N" pb)" "page $P3N"
finish_suite "embed_pointer_keys_browser_seq_e2e"
