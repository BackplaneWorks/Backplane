#!/bin/bash
# An automatic attach (Attach::Automatic: the rail's design mode, a thread opened on its Board page) never takes the keyboard: the person
# types in the composer while KiCad starts and comes on show in the pane, and every key lands in the composer (real X keys, XTest).
#   A  cold, at thread-open: the thread selected, its Board page pressed and the attach asked at once (the viewer has no file yet: the
#      attach is kept and acted on when it has; the pane's warm-up and the attach start KiCad together), typing from that moment for
#      ~13 s (KiCad's start, its first show, the schematic pre-warm at 2 s and the frames change, the 3D models' pre-warm)
#   B  warm: View, then the attach again with KiCad still running (the kept container), typing straight away for ~8 s
#   For each: the composer holds every key typed; no `kicad_auto_keys_taken` (KiCad took the keyboard by itself), no `kicad_keys_flip`
#   to true, no `kicad_ev` focus, no hand-in (`kicad_auto_to_deliberate`); the X input focus, sampled every 100 ms, never in KiCad's
#   container; KiCad's frame on screen over the pane (within 1 px); and it was really on show and painted (else the case proves nothing)
#   E  Edit on an automatic pane already on show (View, the attach again, then Edit): deliberate, KiCad has the keyboard in 3 s
#   C  then a click in the pane: KiCad has the keyboard (the X focus in its container; the pane says so); a click in Backplane's own
#      window (its conversation, left of the pane): back to Backplane (Escape is KiCad's there: embed_lend_e2e L3)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_auto_attach_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub

A_TEXT="the quick brown fox jumps over the lazy dog "
B_TEXT="pack my box with five dozen jugs "
cat >"$OUT/script.json" <<JSON
[{"wait":3000},{"mark":"base_go"},{"wait":2500},{"read":"composer"},{"mark":"base_done"},
 {"select":0},{"press":"rail:Board"},{"kicad":{"attach":"board","why":"auto","tag":"A_attach"}},{"mark":"A_go"},{"wait":24000},
 {"kicad":{"report":true,"tag":"A"}},{"read":"composer"},{"mark":"A_done"},
 {"click":"ui:View"},{"wait":3000},{"kicad":{"attach":"board","why":"auto","tag":"B_attach"}},{"mark":"B_go"},{"wait":16000},
 {"kicad":{"report":true,"tag":"B"}},{"read":"composer"},{"mark":"B_done"},
 {"click":"ui:View"},{"wait":2500},{"kicad":{"attach":"board","why":"auto","tag":"E_attach"}},{"wait":4000},{"mark":"E_go"},{"click":"ui:Edit"},
 {"wait":3000},{"kicad":{"report":true,"tag":"E"}},{"mark":"E_done"},
 {"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 900 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"

# a key per character, paced over the window (KiCad's start and first show come in the middle of it)
type_paced() { local t=$1 gap=$2 i ch
  for ((i = 0; i < ${#t}; i++)); do ch=${t:i:1}; [ "$ch" = " " ] && ch=space; python3 "$XINPUT_PY" key "$ch" >/dev/null; sleep "$gap"; done; }
# the X input focus every 100 ms into $1 until $2 exists
focus_sampler() { while [ ! -e "$2" ]; do echo "$(date +%s%3N) $(xfocus_chain)" >>"$1"; sleep 0.1; done; }
# the composer's text as the script's last read before mark $1
composer_at() { python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
text = None
for l in open(sys.argv[1], errors="replace"):
    if "GPUI_METRIC " not in l: continue
    try: o = json.loads(l.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if o.get("event") == "read" and o.get("what") == "composer": text = o.get("text")
    if o.get("event") == "mark" and o.get("name") == sys.argv[2]: break
print(json.dumps(text))
PY
}
# client log lines between marks $1 and $2 matching $3
between() { python3 - "$OUT/client.log" "$1" "$2" "$3" <<'PY'
import re, sys
on = False; n = 0; first = ""
for l in open(sys.argv[1], errors="replace"):
    if f'"name":"{sys.argv[2]}","event":"mark"' in l: on = True
    if f'"name":"{sys.argv[3]}","event":"mark"' in l: break
    if on and re.search(sys.argv[4], l): n += 1; first = first or l.strip()[:300]
print(n, first)
PY
}

wait_for_log "$OUT/client.log" '"name":"base_go","event":"mark"' 120 || { echo "FAIL the client never reached the thread"; exit 1; }
# the control: the composer has the keyboard before anything (else the cases prove nothing)
type_paced "ok " 0.1
wait_for_log "$OUT/client.log" '"name":"base_done","event":"mark"' 30 || true
BASE=$(composer_at base_done)
check "the composer has the keyboard before any attach (control)" "$([ "$BASE" = '"ok "' ] && echo 1 || echo 0)" "composer $BASE"

run_case() { local c=$1 text=$2 gap=$3
  wait_for_log "$OUT/client.log" "\"name\":\"${c}_go\",\"event\":\"mark\"" 60 || { echo "FAIL case $c never started"; return; }
  rm -f "$OUT/$c.stop"; focus_sampler "$OUT/$c.focus" "$OUT/$c.stop" & local sp=$!
  type_paced "$text" "$gap"
  wait_for_log "$OUT/client.log" "\"name\":\"${c}_done\",\"event\":\"mark\"" 60 || true
  touch "$OUT/$c.stop"; wait "$sp" 2>/dev/null || true
  local r; r=$(last_kicad_report "$OUT/client.log" "$c")
  read -r CONT PAINTED NFRAMES < <(python3 -c 'import json,sys
r=json.loads(sys.argv[1]); c=r.get("container") or {}; s=r.get("shown") or 0; fs=r.get("frames") or []
fr=[f for f in fs if f.get("xid")==s]
print("0x%x" % int(c.get("xid") or 0), (fr[0].get("painted") or 0) if fr else 0, len(fs))' "$r")
  check "$c KiCad's frame was on show and painted (else the case proves nothing)" "$([ "${PAINTED:-0}" -ge 15 ] && echo 1 || echo 0)" "painted ${PAINTED:-0}, container $CONT, frames $NFRAMES"
  # the frame is on screen over the pane: KiCad's shown frame where the pane is in root coordinates, within 1 px (the first suite's check)
  local over; over=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
r=json.loads(sys.argv[1]); wins=json.load(sys.stdin); p=r.get("pane") or {}; s=r.get("shown") or 0
f=next((f for f in r.get("frames") or [] if f.get("xid")==s), {}); top=next((w for w in wins if w["xid_int"]==int(r.get("toplevel") or 0)), None)
if not (p and f and top and f.get("shown")): print(0, "pane", p, "frame", f.get("shown"), "top", bool(top)); sys.exit()
want=(top["root_x"]+p["x"], top["root_y"]+p["y"], p["w"], p["h"]); got=(f.get("root_x"), f.get("root_y"), f.get("w"), f.get("h"))
print(1 if all(g is not None and abs(g-w)<=1 for g,w in zip(got,want)) else 0, "frame", got, "pane", want)' "$r")
  check "$c KiCad's frame is on screen over the pane (within 1 px)" "${over%% *}" "${over#* }"
  local got; got=$(composer_at "${c}_done")
  check "$c every key typed landed in the composer" "$(python3 -c 'import json,sys; t=json.loads(sys.argv[1]) or ""; print(1 if t.endswith(sys.argv[2]) else 0)' "$got" "$text")" "composer $got"
  for pat in kicad_auto_keys_taken '"kicad_keys_flip":\{"to":true' '"kicad_ev":"focus' kicad_auto_to_deliberate; do
    read -r n first < <(between "${c}_go" "${c}_done" "$pat")
    check "$c no $pat" "$([ "$n" = 0 ] && echo 1 || echo 0)" "$n: $first"
  done
  local inkicad; inkicad=$(grep -c "$CONT" "$OUT/$c.focus" || true)
  check "$c the X focus never in KiCad's container ($(wc -l <"$OUT/$c.focus") samples)" "$([ "$CONT" != 0x0 ] && [ "${inkicad:-0}" = 0 ] && echo 1 || echo 0)" "$inkicad in $CONT: $(grep -m1 "$CONT" "$OUT/$c.focus" || true)"
  grep -o '"kicad_attach":"[^"]*","why":"[^"]*"' "$OUT/client.log" | sed -n '$p' | sed "s/^/INFO $c /"
}
run_case A "$A_TEXT" 0.27
read -r n first < <(between A_go A_done '"kicad_info":"[0-9a-f]*","gone":false,"state":"starting"')
check "A was cold: KiCad started inside the typing window" "$([ "$n" -gt 0 ] && echo 1 || echo 0)" "$n starting infos"
run_case B "$B_TEXT" 0.22

# E: Edit on an automatic pane already on show (the deliberate open of what Backplane showed): KiCad has the keyboard within 3 s
wait_for_log "$OUT/client.log" '"name":"E_done","event":"mark"' 60 || true
RE=$(last_kicad_report "$OUT/client.log" E)
read -r n first < <(between E_go E_done '"kicad_auto_to_deliberate":"deliberate"')
check "E Edit makes the automatic pane deliberate" "$([ "$n" -gt 0 ] && echo 1 || echo 0)" "$n"
FOC=$(xfocus_chain)
check "E and KiCad has the keyboard (the X focus in its container; the pane says so)" "$(echo "$FOC" | grep -q "$CONT" && python3 -c 'import json,sys; print(1 if json.loads(sys.argv[1]).get("keys") else 0)' "$RE" | grep -q 1 && echo 1 || echo 0)" "$FOC"
python3 "$XINPUT_PY" key Escape >/dev/null; sleep 0.3
# back to Backplane before C: a click in its own window
C_TOP=$(python3 -c 'import json,sys; print("0x%x" % int(json.loads(sys.argv[1]).get("toplevel") or 0))' "$RE")
C_XY=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid_int"]==int(sys.argv[1],0)]
print("%d %d" % (w[0]["root_x"]+w[0]["w"]//4, w[0]["root_y"]+w[0]["h"]//2) if w else "")' "$C_TOP")
# shellcheck disable=SC2086
[ -n "$C_XY" ] && python3 "$XINPUT_PY" click $C_XY >/dev/null; sleep 1

# C: the person's click in the pane hands the keys over as on any pane; Escape gives them back
R=$(last_kicad_report "$OUT/client.log" E)
read -r SHOWN < <(python3 -c 'import json,sys; print("0x%x" % int(json.loads(sys.argv[1]).get("shown") or 0))' "$R")
XY=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid_int"]==int(sys.argv[1],0)]
print("%d %d" % (w[0]["root_x"]+w[0]["w"]*2//3, w[0]["root_y"]+w[0]["h"]//2) if w else "")' "$SHOWN")
if [ -n "$XY" ]; then
  # shellcheck disable=SC2086
  python3 "$XINPUT_PY" click $XY >/dev/null; sleep 1.5
  FOC=$(xfocus_chain)
  check "C a click in the pane gives KiCad the keyboard" "$(echo "$FOC" | grep -q "$CONT" && grep -q '"kicad_keys_flip":{"to":true' "$OUT/client.log" && echo 1 || echo 0)" "$FOC"
  GXY=$(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys
w=[w for w in json.load(sys.stdin) if w["xid_int"]==int(sys.argv[1],0)]
print("%d %d" % (w[0]["root_x"]+w[0]["w"]//4, w[0]["root_y"]+w[0]["h"]//2) if w else "")' "$(python3 -c 'import json,sys; print("0x%x" % int(json.loads(sys.argv[1]).get("toplevel") or 0))' "$R")")
  # shellcheck disable=SC2086
  [ -n "$GXY" ] && python3 "$XINPUT_PY" click $GXY >/dev/null; sleep 1.5
  FOC=$(xfocus_chain)
  check "C a click in Backplane's own window gives it back to Backplane" "$([ -n "$GXY" ] && ! echo "$FOC" | grep -q "$CONT" && echo 1 || echo 0)" "$FOC"
else
  check "C a click in the pane gives KiCad the keyboard" 0 "no frame on show ($SHOWN)"
fi
finish_suite "embed_auto_attach_e2e ($EMBED_MODE)"
