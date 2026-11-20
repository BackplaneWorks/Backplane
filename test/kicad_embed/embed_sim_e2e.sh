#!/bin/bash
# KiCad's Simulator as the pane's "Simulation" tab (bp.13.2, capability sim-frame; docs/embedded-editors.md "Simulation"). Fork only.
#   S1  the rail has Simulation for a project with a schematic; pressed, the pane's KiCad opens on the Simulator
#   S2  its frame is a child of the pane, at the pane's rectangle, painted; the strip lists its tab
#   S3  a click gives it the keyboard; Backplane's own chord (Ctrl+K: the palette) still works from it, and the keys go back after
#   S4  Schematic and back (the strip's tabs, then the rail): the same Simulator window each time, nothing rebuilt
#   L1-L4  the rail entry drawn lit follows the frame on show (the client's own word, GPUI_METRIC "rail"): Simulation while the Simulator is
#       shown, Schematic after the strip's Schematic tab, Simulation again by the rail, not Simulation once it is closed
#   S5  Ctrl+W in the Simulator closes it: the pane is back on View (Edit offered); the rail's Simulation opens it again
#   SIM_OLD=1: a runtime without sim-frame (SIM_OLD_KICAD, else the newest one installed here; none is a failure): no Simulation on the rail, no tab in the strip,
#   Ctrl+Tab never goes to it, the editors work as before
# XTest as a person types and clicks. A runtime without sim-frame skips S1-S5 (they need bp.13.2).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_sim_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
OLD=${SIM_OLD:-0}
if [ "$OLD" = 1 ]; then
  # the runtime asked for (SIM_OLD_KICAD), else the newest fork runtime installed here that has no sim-frame. None is a FAILURE, not a skip:
  # a run that tested nothing once passed for "8/0" (h@backplane has no runtime under ~/.backplane*).
  KICAD_BIN=${SIM_OLD_KICAD:-}
  if [ -z "$KICAD_BIN" ]; then
    for cand in $(ls -1dr "$REAL_HOME"/.backplane*/home/kicad/10.99.0-bp.*/bin/kicad 2>/dev/null || true); do
      "$(dirname "$cand")/kicad-cli" api-server --describe 2>/dev/null | grep -q '"sim-frame"' || { KICAD_BIN=$cand; break; }
    done
  fi
  [ -n "$KICAD_BIN" ] && [ -x "$KICAD_BIN" ] || { echo "FAIL embed_sim_e2e (old runtime): no KiCad without sim-frame to run it on (${KICAD_BIN:-none under $REAL_HOME/.backplane*/home/kicad}): nothing was checked. Set SIM_OLD_KICAD=<runtime>/bin/kicad (a bp.12.x or bp.13.1; remote.sh ships it)"; exit 1; }
  export KICAD_BIN
fi
discover_bins; discover_kicad; init_isolated_dirs
caps_of() { "$1" api-server --describe 2>/dev/null | python3 -c 'import json,sys; print(" ".join(json.load(sys.stdin).get("capabilities", [])))' 2>/dev/null || true; }
RT=$(cd "$(dirname "$KICAD_BIN")/.." && pwd)
HAS=$(caps_of "$KICAD_CLI" | tr ' ' '\n' | grep -cx sim-frame || true)
echo "RUNTIME $("$KICAD_CLI" --version 2>/dev/null | tail -1) at $RT; sim-frame: $([ "$HAS" = 1 ] && echo yes || echo no)"
if [ "$OLD" = 1 ] && [ "$HAS" = 1 ]; then echo "FAIL the old runtime asked for ($RT) says sim-frame"; exit 1; fi
if [ "$OLD" != 1 ] && [ "$HAS" != 1 ]; then echo "embed_sim_e2e: $RT has no sim-frame (needs Backplane KiCad bp.13.2): skipped"; exit 0; fi
# a development runtime whose manifest.json is older than its programs (it lacks sim-frame, which its kicad-cli says): the hub reads the
# manifest, as it does a released package's, so the run uses a view of the runtime with the manifest its kicad-cli describes
if [ "$OLD" != 1 ] && ! python3 -c 'import json,sys; sys.exit(0 if "sim-frame" in json.load(open(sys.argv[1])).get("capabilities", []) else 1)' "$RT/manifest.json" 2>/dev/null; then
  mkdir -p "$OUT/rt"
  for e in "$RT"/*; do [ "$(basename "$e")" = manifest.json ] || ln -sfn "$e" "$OUT/rt/$(basename "$e")"; done
  "$KICAD_CLI" api-server --describe >"$OUT/rt/manifest.json"
  echo "ENV the runtime's manifest.json lacks sim-frame (stale beside newer programs): running through $OUT/rt with the manifest its kicad-cli describes"
  KICAD_BIN="$OUT/rt/bin/kicad"; KICAD_CLI="$OUT/rt/bin/kicad-cli"; export KICAD_BIN KICAD_CLI
fi
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub

ctl_at() { python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
last = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"controls":[' in l:
        try: last = json.loads(l.split("GPUI_METRIC ", 1)[1])["controls"]
        except Exception: pass
c = next((c for c in (last or []) if c.get("id") == sys.argv[2] and c.get("rect")), None)
print(f'{int(c["rect"]["x"] + c["rect"]["w"] / 2)} {int(c["rect"]["y"] + c["rect"]["h"] / 2)}' if c else "")
PY
}
# the rail as the client drew it last (GPUI_METRIC event "rail", under BACKPLANE_GPUI_RECTS): "$1" = lit | entries | disabled, labels joined by "|";
# "none" when the client has said nothing of its rail yet
rail() { python3 -c 'import json, sys
last = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and "\"event\":\"rail\"" in l:
        try: last = json.loads(l.split("GPUI_METRIC ", 1)[1])["rail"]
        except Exception: pass
print("none" if last is None else "|".join(last.get(sys.argv[2], [])))' "$OUT/client.log" "$1"; }
# 1 when, within 5 s, the entry $1 is lit and none of the others named is ("" for $1: only that the others are not lit)
lit_is() { local l ok n; for _ in $(seq 20); do l=$(rail lit); ok=1; [ "$l" = none ] && ok=0
    [ -z "$1" ] || case "|$l|" in *"|$1|"*) ;; *) ok=0 ;; esac
    for n in "${@:2}"; do case "|$l|" in *"|$n|"*) ok=0 ;; esac; done
    [ "$ok" = 1 ] && { echo 1; return; }; sleep 0.25; done; echo 0; }
press() { local at; at=$(ctl_at "$1"); [ -n "$at" ] || return 1; python3 "$XINPUT_PY" click $at; }
# KiCad's frames by its own word (GetFrames): "<window> <shown 0|1> <focused 0|1>" of FrameKind $1, "" when it has none
frame() { python3 "$IPC_PY" frames "$SOCK" 2>/dev/null | python3 -c 'import json,sys
f=next((f for f in json.load(sys.stdin).get("frames", []) if f["kind"]==int(sys.argv[1])), None)
print(f["window"], int(f["shown"]), int(f["focused"])) if f else print("")' "$1" 2>/dev/null || echo ""; }
wait_frame() { # $1 kind, $2 shown 0|1|gone, $3 tries of 0.25 s
  local f; for _ in $(seq "${3:-80}"); do f=$(frame "$1"); if [ "$2" = gone ]; then [ -z "$f" ] && return 0; else [ -n "$f" ] && [ "$(echo "$f" | cut -d' ' -f2)" = "$2" ] && return 0; fi; sleep 0.25; done; return 1; }
# the window's place on the root and its size, by X: "x y w h viewable"
geom() { python3 "$DRIVER_PY" list-windows 2>/dev/null | python3 -c 'import json,sys
x=lambda v: v if isinstance(v, int) else int(str(v), 0)
w=next((w for w in json.load(sys.stdin) if x(w["xid"])==int(sys.argv[1])), None)
print(w["root_x"], w["root_y"], w["w"], w["h"], int(bool(w.get("shown")))) if w else print("")' "$1" 2>/dev/null || echo ""; }
FK_SCH=2; FK_SIM=8

if [ "$OLD" = 1 ]; then
  echo '[{"select":0},{"wait":2200},{"click":"rail:Schematic"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":90000,"kind":"SCH","tag":"ready"}},{"mark":"ready"},{"wait":1500000}]' >"$OUT/script.json"
else
  echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":4000},{"mark":"page"},{"wait":1500},{"click":"rail:Simulation"},{"kicad":{"wait_painted":90000,"kind":"SIMULATOR","tag":"sim"}},{"mark":"sim"},{"wait":1500000}]' >"$OUT/script.json"
fi
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1600 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"

if [ "$OLD" = 1 ]; then
  wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the schematic never opened in KiCad"; exit 1; }
  R=$(last_kicad_report "$OUT/client.log" ready)
  SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock",""))' "$R"); KP=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid",0))' "$R")
  sleep 2
  echo "INFO the embedded KiCad says: $(python3 "$IPC_PY" capabilities "$SOCK" 2>/dev/null | python3 -c 'import json,sys; print(" ".join(c for c in json.load(sys.stdin).get("capabilities", []) if c.startswith(("sim", "embed"))))' 2>/dev/null)"
  check "O1 an older runtime: the schematic editor opens in the pane as before" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(1 if r.get("state")=="ready" and r.get("in_gpui_tree") and r.get("want")=="SCH" else 0)' "$R")"
  check "O2 no Simulation on the rail" "$(grep -q '"id":"rail:Simulation"' "$OUT/client.log" && echo 0 || echo 1)"
  check "O3 no Simulation tab in the strip (the other tabs are there)" "$([ -z "$(ctl_at kicad-tab:SIMULATOR)" ] && [ -n "$(ctl_at kicad-tab:SCH)" ] && echo 1 || echo 0)" "SCH tab at $(ctl_at kicad-tab:SCH)"
  check "O6 the rail as drawn never has Simulation (not an entry, not lit) and lights Schematic while the schematic editor shows" "$([ "$(grep -c '"event":"rail"' "$OUT/client.log")" -gt 0 ] && ! grep '"event":"rail"' "$OUT/client.log" | grep -q Simulation && [ "$(lit_is Schematic Board)" = 1 ] && echo 1 || echo 0)" "lit $(rail lit); entries $(rail entries)"
  # Ctrl+Tab round the tabs from KiCad: never the Simulator
  read -r F _ _ <<<"$(frame $FK_SCH)"; read -r GX GY GW GH _ <<<"$(geom "$F")"
  python3 "$XINPUT_PY" click $((GX + GW / 2)) $((GY + GH - 120)); sleep 0.8
  N=$(wc -l <"$OUT/client.log")
  for _ in 1 2 3 4 5 6 7; do python3 "$XINPUT_PY" chord ctrl Tab; sleep 1.2; done
  TABS=$(tail -n +"$N" "$OUT/client.log" | grep -o '"kicad_tab":{[^}]*' | grep -o '"to":"[A-Z_0-9]*"' | tr '\n' ' ')
  check "O4 Ctrl+Tab goes round the editors and never to a Simulator this KiCad has not" "$([ -n "$TABS" ] && ! echo "$TABS" | grep -q SIMULATOR && echo 1 || echo 0)" "$TABS"
  check "O7 after the rounds of Ctrl+Tab the drawn rail still never had Simulation" "$(grep '"event":"rail"' "$OUT/client.log" | grep -q Simulation && echo 0 || echo 1)" "lit $(rail lit)"
  check "O5 nothing was refused or failed" "$(grep -q 'no simulation\|"state":"failed"' "$OUT/client.log" "$OUT/hub.log" && echo 0 || echo 1)"
  shot old-runtime
  kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
  finish_suite "embed_sim_e2e old runtime ($EMBED_MODE)"; exit $?
fi

# S1
wait_for_log "$OUT/client.log" '"name":"page","event":"mark"' 240 || { echo "FAIL the thread's page never came"; exit 1; }
check "S1 the rail has Simulation beside Board and Schematic (the hub's word: this KiCad has sim-frame, the project a schematic)" "$([ -n "$(ctl_at rail:Simulation)" ] && echo 1 || echo 0)" "at $(ctl_at rail:Simulation)"
wait_for_log "$OUT/client.log" '"name":"sim","event":"mark"' 480 || { echo "FAIL the Simulator never opened in the pane"; tail -5 "$OUT/hub.log"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" sim)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock",""))' "$R"); KP=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid",0))' "$R")
echo "INFO the embedded KiCad says: $(python3 "$IPC_PY" capabilities "$SOCK" 2>/dev/null | python3 -c 'import json,sys; print(" ".join(c for c in json.load(sys.stdin).get("capabilities", []) if c.startswith(("sim", "embed-request"))))' 2>/dev/null)"
echo "INFO KiCad's frames: $(python3 "$IPC_PY" frames "$SOCK" 2>/dev/null | python3 -c 'import json,sys; print([(f["kind"], f["shown"], f["title"]) for f in json.load(sys.stdin)["frames"]])' 2>/dev/null)"
check "S1 pressed, the pane's KiCad opens on the Simulator" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(1 if r.get("state")=="ready" and r.get("want")=="SIMULATOR" and r.get("shown") else 0)' "$R")" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print({k: r.get(k) for k in ("state","want","shown","say","wait_ms")})' "$R")"
# S2
sleep 1.5; shot sim-tab
read -r SIMW _ _ <<<"$(frame $FK_SIM)"
check "S2 the Simulator's frame is a child of the pane's container in Backplane's window, at the pane's rectangle, painted" "$(python3 -c '
import json,sys
r=json.loads(sys.argv[1]); p=r.get("pane") or {}; c=r.get("container") or {}
f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")), {})
ok = r.get("in_gpui_tree") is True and f.get("kind")=="SIMULATOR" and f.get("shown") is True and f.get("xid")==int(sys.argv[2] or 0) \
  and abs(c.get("x",-99)-p.get("x",0))<=2 and abs(c.get("y",-99)-p.get("y",0))<=2 and abs(f.get("w",-99)-p.get("w",0))<=2 and abs(f.get("h",-99)-p.get("h",0))<=2 \
  and p.get("w",0)>300 and f.get("painted",0)>=15
print(1 if ok else 0)' "$R" "${SIMW:-0}")" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")), {}); print("pane", r.get("pane"), "container", {k: (r.get("container") or {}).get(k) for k in ("x","y","w","h")}, "frame", {k: f.get(k) for k in ("kind","x","y","w","h","painted","shown")}, "in tree", r.get("in_gpui_tree"))' "$R")"
check "L1 while the Simulator is shown the rail's Simulation entry is the lit one; Schematic and Board are not" "$(lit_is Simulation Schematic Board)" "lit $(rail lit); entries $(rail entries); disabled $(rail disabled)"
check "S2 the strip lists the Simulation tab" "$([ -n "$(ctl_at kicad-tab:SIMULATOR)" ] && echo 1 || echo 0)"
read -r GX GY GW GH _ <<<"$(geom "$SIMW")"
# S3: a click in its console (low left: a click on the signal grid would change a trace), then Backplane's chord
python3 "$XINPUT_PY" click $((GX + GW / 5)) $((GY + GH * 3 / 4)); sleep 1.0
check "S3 a click gives the Simulator the keyboard" "$([ "$(frame $FK_SIM | cut -d' ' -f3)" = 1 ] && echo 1 || echo 0)" "frame $(frame $FK_SIM)"
N=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl k; sleep 1.2
check "S3 Ctrl+K pressed in the Simulator opens Backplane's palette" "$(tail -n +"$N" "$OUT/client.log" | grep -q '"kicad_key_dispatch":"ctrl-k"' && echo 1 || echo 0)" "$(tail -n +"$N" "$OUT/client.log" | grep -o '"kicad_key[^}]*' | head -2 | tr '\n' ' ')"
shot sim-palette
python3 "$XINPUT_PY" key Escape; sleep 1.2
check "S3 Escape shuts the palette and the Simulator has the keyboard again" "$([ "$(frame $FK_SIM | cut -d' ' -f3)" = 1 ] && echo 1 || echo 0)" "frame $(frame $FK_SIM)"
check "S3 the Simulator is still there (the chord was Backplane's, not KiCad's)" "$([ "$(frame $FK_SIM | cut -d' ' -f1)" = "$SIMW" ] && echo 1 || echo 0)"
# S4: the strip's Schematic tab and back, then the rail's Schematic and Simulation
T0=$(date +%s%N); press kicad-tab:SCH || true
wait_frame $FK_SCH 1 120 && SCH_OK=1 || SCH_OK=0
check "S4 the strip's Schematic tab shows the schematic editor; the Simulator is kept, hidden" "$([ "$SCH_OK" = 1 ] && [ "$(frame $FK_SIM | cut -d' ' -f1)" = "$SIMW" ] && echo 1 || echo 0)" "sch $(frame $FK_SCH) sim $(frame $FK_SIM)"
check "L2 after the strip's Schematic tab the rail lights Schematic, not Simulation" "$(lit_is Schematic Simulation)" "lit $(rail lit)"
sleep 2.5; shot sim-schematic
T0=$(date +%s%N); press kicad-tab:SIMULATOR || true
for _ in $(seq 80); do G=$(geom "$SIMW"); [ "$(echo "$G" | cut -d' ' -f5)" = 1 ] && break; sleep 0.05; done
MS=$(( ($(date +%s%N) - T0) / 1000000 ))
check "S4 back on the Simulation tab: the same window (nothing rebuilt), at the pane's rectangle" "$([ "$(frame $FK_SIM | cut -d' ' -f1)" = "$SIMW" ] && [ "$G" = "$GX $GY $GW $GH 1" ] && echo 1 || echo 0)" "window $(frame $FK_SIM) was $SIMW; at $G was $GX $GY $GW $GH; ${MS} ms"
echo "INFO Schematic -> Simulation by the strip's tab: ${MS} ms to the Simulator's window on show (software GL under Xvfb unless REAL_HARDWARE=1)"
sleep 1; press rail:Schematic || true
for _ in $(seq 80); do [ "$(geom "$SIMW" | cut -d' ' -f5)" = 0 ] && break; sleep 0.1; done
check "S4 the rail's Schematic takes the pane to the schematic editor" "$([ "$(geom "$SIMW" | cut -d' ' -f5)" = 0 ] && [ -n "$(frame $FK_SCH)" ] && echo 1 || echo 0)" "sch $(frame $FK_SCH) sim $(geom "$SIMW")"
sleep 2.5; press rail:Simulation || true
for _ in $(seq 80); do G=$(geom "$SIMW"); [ "$(echo "$G" | cut -d' ' -f5)" = 1 ] && break; sleep 0.1; done
check "S4 the rail's Simulation brings the same Simulator back" "$([ "$(frame $FK_SIM | cut -d' ' -f1)" = "$SIMW" ] && [ "$(echo "$G" | cut -d' ' -f5)" = 1 ] && echo 1 || echo 0)" "window $(frame $FK_SIM) was $SIMW; $G"
check "L3 back on the Simulator by the rail: Simulation is lit again; Schematic and Board are not" "$(lit_is Simulation Schematic Board)" "lit $(rail lit)"
check "S4 one KiCad through it all" "$(kill -0 "$KP" 2>/dev/null && [ "$(grep -c 'kicad embed: running ' "$OUT/hub.log")" = 1 ] && echo 1 || echo 0)"
# S5: Ctrl+W
sleep 1.5; read -r GX GY GW GH _ <<<"$(geom "$SIMW")"
python3 "$XINPUT_PY" click $((GX + GW / 5)) $((GY + GH * 3 / 4)); sleep 1.0
N=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" chord ctrl w
wait_frame $FK_SIM gone 60 && GONE=1 || GONE=0; sleep 2
check "S5 Ctrl+W in the Simulator closes it (no prompt: nothing unsaved)" "$GONE" "frame $(frame $FK_SIM)"
check "S5 the pane is back on View (Edit offered again)" "$(tail -n +"$N" "$OUT/client.log" | grep -q '"kicad_frame_closed":"SIMULATOR"' && [ -n "$(ctl_at ui:Edit)" ] && echo 1 || echo 0)" "$(tail -n +"$N" "$OUT/client.log" | grep -o '"kicad_frame_closed[^}]*' | head -1); Edit at $(ctl_at ui:Edit)"
check "L4 with the Simulator closed its entry is no longer lit" "$(lit_is "" Simulation)" "lit $(rail lit)"
shot sim-closed
press rail:Simulation || true
wait_frame $FK_SIM 1 160 && BACK=1 || BACK=0; sleep 2
read -r SIM2 _ _ <<<"$(frame $FK_SIM)"; G=$(geom "${SIM2:-0}")
check "S5 the rail's Simulation opens it again, in the pane" "$([ "$BACK" = 1 ] && [ -n "$SIM2" ] && [ "$(echo "$G" | cut -d' ' -f3-5)" = "$GW $GH 1" ] && kill -0 "$KP" 2>/dev/null && echo 1 || echo 0)" "window ${SIM2:-none} at $G (was $GX $GY $GW $GH)"
shot sim-reopened
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_sim_e2e ($EMBED_MODE)"
