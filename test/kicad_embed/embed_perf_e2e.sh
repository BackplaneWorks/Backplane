#!/bin/bash
# Timings (stock|fork; KEMBED_PROJ_SRC for a real project's copy): Edit's cold start, then each tab switch, View and Edit again (KiCad warm) (the click to the editor
# shown and drawn: Board, Schematic, 3D, back), and a save (Ctrl+S pressed in KiCad to the file written, KiCad clean and answering).
# Prints PERF lines; checks the budgets below (KEMBED_PERF_STRICT=0 only prints).
# The warm-ups are checked to have happened (fork): the pre-Edit warm-up (editor.kicad.warm answered started, no "not warming" in the hub
# log) before "no KiCad window before Edit" counts, and the schematic loaded hidden (its frame there, not shown, before the first switch to
# it: by the post-Edit pre-warm or, on bp.13.2, by the hub's checks' DRC parity) before "the pre-warmed schematic is not modified" counts;
# that state is read before the first switch and must be answered. KEMBED_FORCE_REFUSE=warm forces a refused warm-up (to see the first
# fail); =prewarm refuses the pre-warm, which on bp.13.2 the checks' hidden load makes up for.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "spike" ] && { echo "embed_perf_e2e: stock|fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
case "${KEMBED_FORCE_REFUSE:-}" in
  warm) export BACKPLANE_KICAD_WARM_HEADROOM_MB=10000000 ;;      # no machine has 10 TB free: the pre-Edit warm-up is refused
  prewarm) export BACKPLANE_KICAD_PREWARM_FREE_MB=10000000 ;;     # the post-Edit pre-warm is refused
  "") ;;
  *) echo "KEMBED_FORCE_REFUSE is warm or prewarm"; exit 2 ;;
esac
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "cold"}}, {"wait": 4000}]
# the pre-warmed schematic as the pre-warm left it, before any switch to it: the shell reads its modified state in the pause after the mark
s += [{"kicad": {"report": True, "tag": "prewarmed"}}, {"mark": "prewarmed"}, {"wait": 5000}]
for i, k in enumerate(["SCH", "PCB_3D", "PCB", "SCH", "PCB", "PCB_3D", "SCH"]):
    s += [{"click": f"kicad-tab:{k}"}, {"kicad": {"wait_painted": 30000, "kind": k, "tag": f"sw{i}_{k}"}}, {"wait": 1500}]
# back to the viewer and Edit again: KiCad is warm in the hub (the pre-warmed Edit)
s += [{"click": "kicad-tab:PCB"}, {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "before_view"}}, {"wait": 800}, {"click": "ui:View"}, {"wait": 1500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "warm_edit"}}, {"wait": 1500}]
s += [{"click": "kicad-tab:PCB"}, {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "save_ready"}}, {"mark": "save"}, {"wait": 25000}, {"click": "ui:View"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
# until Edit is pressed no KiCad window may be on screen (the hub warms KiCad when the project is shown: it must stay out of sight)
python3 - "$OUT/client.log" "$KEMBED_LIB" >"$OUT/preedit.txt" 2>&1 <<'PY' &
import os, sys, time, json
log, lib_dir = sys.argv[1], sys.argv[2]; sys.path.insert(0, lib_dir)
import x11_embed_driver as d
lib = d.load_lib(); seen = {}; t0 = time.time()
while time.time() - t0 < 90:
    # the press itself, not the controls/rects inventory that names the button (BACKPLANE_GPUI_RECTS=1 logs it from the start)
    if os.path.exists(log) and any('"id":"ui:Edit","changed"' in l and '"event":"click"' in l for l in open(log, errors="ignore")): break
    for w in d.list_all_windows(lib):
        k = (w["title"] + " " + w["wm_class"]).lower()
        if w["shown"] and not w["override_redirect"] and w["w"] >= 50 and ("kicad" in k or "editor" in k) and "backplane" not in k: seen.setdefault(w["xid"], w["title"])
    time.sleep(0.05)
print(json.dumps(seen))
PY
FLASH_PID=$!; track_pid "$FLASH_PID"
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the pre-warmed schematic's modified state, read in the pause after the "prewarmed" mark (before the first Schematic tab switch)
PRE=n/a
for _ in $(seq 900); do grep -q '"name":"prewarmed","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
if grep -q '"name":"prewarmed","event":"mark"' "$OUT/client.log" 2>/dev/null; then
  PSOCK=$(last_kicad_report "$OUT/client.log" prewarmed | python3 -c 'import json,sys; print(json.load(sys.stdin).get("sock") or "-")')
  PRE=$(python3 "$IPC_PY" modified "$PSOCK" sch 2>/dev/null | python3 -c 'import json,sys; print({True: "yes", False: "no"}.get(json.load(sys.stdin).get("modified"), "n/a"))' 2>/dev/null || echo n/a)
  # still before the first switch (the client's 5 s pause): else the read is not the pre-warm's state
  # the tab's click, not the controls/rects inventory that names the tab (BACKPLANE_GPUI_RECTS=1 logs it before the mark: every read
  # was thrown away as "after the tab was shown", rail-k1's runs from 2727a49894 on)
  grep '"id":"kicad-tab:SCH","changed"' "$OUT/client.log" | grep -q '"event":"click"' && PRE="n/a (read after the Schematic tab was shown)"
fi
for _ in $(seq 1500); do grep -q '"name":"save","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" save_ready)
read -r SOCK X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
PCB=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(next((f.get("doc","") for f in r.get("frames",[]) if f.get("kind")=="PCB"), ""))' "$R")
[ -n "$PCB" ] || PCB=$(find "$OUT/proj" -name "*.kicad_pcb" -not -path "*/.git/*" | head -1 || true)
wait "$FLASH_PID" 2>/dev/null || true
PRE_EDIT=$(tail -1 "$OUT/preedit.txt" 2>/dev/null || echo '?')
echo "PERF pre_edit_kicad_windows=$PRE_EDIT"
# did the warm-ups happen: the client's log (the warm asked before Edit, its reply; the pre-warm's reply) and the hub's refusals
IFS=$'\t' read -r WARMED WARM_SAY PREWARMED PREWARM_SAY < <(python3 - "$OUT/client.log" "$OUT/hub.log" "$(last_kicad_report "$OUT/client.log" prewarmed)" <<'PY'
import json, sys
client, hub, rep = sys.argv[1], sys.argv[2], json.loads(sys.argv[3] or "{}")
asked_before_edit = False; edit = False; warm = prewarm = None
for l in open(client, errors="ignore"):
    if '"id":"ui:Edit","changed"' in l and '"event":"click"' in l: edit = True
    if "GPUI_LOG " not in l: continue
    try: o = json.loads(l.split("GPUI_LOG ", 1)[1])
    except Exception: continue
    if o.get("kicad_rpc_sent") == "editor.kicad.warm" and not edit: asked_before_edit = True
    if o.get("kicad_rpc_reply") == "editor.kicad.warm" and warm is None: warm = o
    if o.get("kicad_rpc_reply") == "editor.kicad.prewarm" and prewarm is None: prewarm = o
h = open(hub, errors="ignore").read()
refused = [l for l in h.splitlines() if "kicad embed: not warming" in l]
pre_refused = [l for l in h.splitlines() if "not pre-warming" in l]
w_ok = asked_before_edit and bool(warm) and warm.get("started") is True and not refused
# the schematic loaded hidden before the first switch: its frame there and not shown; by the client's pre-warm, or (bp.13.2) by the hub's
# own checks, whose DRC parity loads it hidden whatever the pre-warm's memory floor says (then the pre-warm finds it there and builds nothing)
sch = any(f.get("kind") == "SCH" and not f.get("shown") for f in rep.get("frames", []))
by = [w for w, yes in (("the pre-warm", bool(prewarm) and "SCH" in (prewarm.get("prewarming") or [])),
                       ("the checks' DRC parity", "parity loads the schematic hidden" in h)) if yes]
p_ok = sch
say = lambda *a: " ".join(str(x) for x in a)[:300].replace("\t", " ")
print(int(w_ok), say("asked before Edit:", asked_before_edit, "reply:", warm and {k: warm.get(k) for k in ("ok", "warm", "started", "why")}, "|", refused[0] if refused else ""),
      int(p_ok), say("SCH hidden:", sch, "by:", " or ".join(by) or "?", "pre-warm reply:", prewarm and {k: prewarm.get(k) for k in ("ok", "prewarming", "error")}, "|", pre_refused[0] if pre_refused else ""), sep="\t")
PY
)
echo "PERF warm=$WARMED ($WARM_SAY)"
echo "PERF prewarm=$PREWARMED ($PREWARM_SAY)"
if [ "$EMBED_MODE" = fork ]; then
  check "the warm-up started KiCad before Edit" "$WARMED" "$WARM_SAY"
  # with no warm-up no KiCad runs before Edit: the window check proves nothing then
  check "no KiCad window on screen before Edit, with KiCad warm" "$([ "$WARMED" = 1 ] && [ "$PRE_EDIT" = "{}" ] && echo 1 || echo 0)" "warmed $WARMED, windows $PRE_EDIT"
  check "the schematic was loaded hidden before the first switch to it" "$PREWARMED" "$PREWARM_SAY"
  # the pre-warm (the schematic opened hidden after the board was shown) must not leave it modified: a load-time repair or notice counts.
  # Read before any switch to it; an unanswered read (n/a) fails
  echo "PERF prewarm_sch_modified=$PRE"
  check "the pre-warmed schematic is not modified" "$([ "$PREWARMED" = 1 ] && [ "$PRE" = no ] && echo 1 || echo 0)" "prewarmed $PREWARMED, modified $PRE"
else
  # stock: no warm-up (it shows its own load windows) and no pre-warm; nothing to check but that no window flashes
  echo "KNOWN stock: no warm-up or pre-warm (warm=$WARMED: $WARM_SAY)"
  check "no KiCad window on screen before Edit" "$([ "$PRE_EDIT" = "{}" ] && echo 1 || echo 0)" "$PRE_EDIT"
fi
# an edit (an agent's track over IPC) so there is something to save, then the person's Ctrl+S with the keyboard in KiCad
python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 110 100 0.25 >/dev/null 2>&1 || true; sleep 1
python3 "$XINPUT_PY" click $((X + W * 3 / 4)) $((Y + H - 40)); sleep 0.5
M0=$(stat -c %.6Y "$PCB")
T0=$(date +%s.%N)
python3 "$XINPUT_PY" chord ctrl s
python3 - "$PCB" "$M0" "$T0" "$SOCK" "$IPC_PY" <<'PY'
import os, subprocess, sys, time, json
pcb, m0, t0, sock, ipc = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), sys.argv[4], sys.argv[5]
written = clean = None
while time.time() - t0 < 30:
    now = time.time() - t0
    if written is None and os.path.exists(pcb) and os.stat(pcb).st_mtime > m0: written = now
    if written is not None:
        out = subprocess.run(["python3", ipc, "modified", sock, "pcb"], capture_output=True, text=True).stdout
        try:
            if json.loads(out).get("modified") is False: clean = time.time() - t0; break
        except Exception: pass
    time.sleep(0.02)
print(f"PERF save_written_ms={int((written or -1)*1000)} save_clean_ms={int((clean or -1)*1000)}")
# how long KiCad answers a Ping, every 200 ms for 6 s after the save: a stall shows as a slow answer
pings = []
t1 = time.time()
while time.time() - t1 < 6:
    a = time.time(); subprocess.run(["python3", ipc, "ping", sock], capture_output=True); pings.append(int((time.time() - a) * 1000)); time.sleep(0.2)
print("PERF save_ping_ms " + " ".join(map(str, pings)))
PY
wait "$CP" 2>/dev/null || true
python3 - "$OUT/client.log" <<'PY'
import json, sys
for l in open(sys.argv[1], errors="ignore"):
    if '"event":"kicad"' in l and '"wait_ms"' in l:
        r = json.loads(l[l.index("{"):]); print(f"PERF {r.get('tag')} {r.get('wait_ms')}ms state={r.get('state')}")
PY
grep -o "kicad embed: start +[0-9]*ms .*" "$OUT/hub.log" | sed "s/^kicad embed: start /PERF cold_phase /" || true
finish_suite "embed_perf_e2e ($EMBED_MODE)"
