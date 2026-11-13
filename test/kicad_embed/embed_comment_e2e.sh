#!/bin/bash
# Suite 7 (fork mode, Backplane KiCad bp.10 or later: SetReservedKeys and CommentSelection; skipped otherwise): Backplane's comment window.
#  - with the keyboard in the embedded board editor and two parts selected there, Ctrl+; (real X input, KiCad's reserved-chord path) opens
#    Backplane's comment window over the pane (how long from the key to its first paint: `open_ms`), and KiCad shows no dialog of its own;
#  - the selection's names come in (the capture: CommentSelection{capture_only} on bp.13, a noteless one whose event is swallowed before);
#  - typed note + Enter: the thread gets ONE message with the note, the names and the screenshot as an attachment, and the keyboard is back in KiCad.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_comment_e2e: fork only"; exit 0; }
discover_bins
discover_kicad
# the runtime this run is on (the hub takes the GUI kicad from beside BACKPLANE_KICAD_CLI: both must name the same runtime)
echo "embed_comment_e2e: KiCad $("$KICAD_CLI" --version 2>/dev/null) (cli $KICAD_CLI, bin $KICAD_BIN)"
[ "$(dirname "$KICAD_CLI")" = "$(dirname "$KICAD_BIN")" ] || { echo "FAIL BACKPLANE_KICAD_CLI and BACKPLANE_KICAD_BIN name different runtimes"; exit 1; }
DESC="$(dirname "$KICAD_BIN")/../share/backplane/api.desc"
if ! grep -qa "CommentSelection" "$DESC" 2>/dev/null || ! grep -qa "SetReservedKeys" "$DESC" 2>/dev/null; then echo "embed_comment_e2e: SKIP (this KiCad has no CommentSelection/SetReservedKeys)"; exit 0; fi
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"wait": 15000}]
for m, w in [("ready", 25000), ("posted", 20000)]:  # posted: the handoff checks run after it (bp.13 batch 4)
    s += [{"mark": m}, {"kicad": {"report": True, "tag": m}}, {"wait": w}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 150 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_mark() { for _ in $(seq 600); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }
rep() { last_kicad_report "$OUT/client.log" "$1"; }
field() { python3 -c 'import json,sys; r=json.loads(sys.argv[1]); v=r.get(sys.argv[2]); print(v if v is not None else "")' "$1" "$2"; }
klog() { { grep -o "GPUI_LOG {\"kcomment\":\"$1\"[^}]*}" "$OUT/client.log" || true; } | tail -1 | sed 's/^GPUI_LOG //'; }
now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }
wait_klog() { for _ in $(seq 100); do [ -n "$(klog "$1")" ] && return 0; sleep 0.05; done; return 1; }

wait_mark ready || true; sleep 1; R=$(rep ready); SOCK=$(field "$R" sock); SHOWN=$(field "$R" shown)
check "the board editor is embedded and shown" "$([ -n "$SOCK" ] && [ -n "$SHOWN" ] && echo 1 || echo 0)" "sock=$SOCK shown=$SHOWN"
# the keyboard into KiCad (a click on its canvas, as the person gives it), then two parts selected there; after the other editors' pre-warm
# (2 s after the first is placed), which a click before it races. One click: it reaches KiCad directly (embed_click_e2e.sh)
sleep 4; click_win_center "$SHOWN"; sleep 0.8
grep "reserved keys" "$OUT/hub.log" | tail -1 || true
SEL=$(python3 "$IPC_PY" select-items "$SOCK" pcb "R1,C1" 0 2>>"$OUT/ipc.err" || echo '{}'); echo "$SEL" >"$OUT/select.json"
sleep 0.5
# the chord pressed in KiCad: its reserved-chord path hands it to Backplane
xfocus() { python3 -c 'import ctypes,ctypes.util,os
x=ctypes.CDLL(ctypes.util.find_library("X11")); x.XOpenDisplay.restype=ctypes.c_void_p; d=ctypes.c_void_p(x.XOpenDisplay(os.environ["DISPLAY"].encode()))
w=ctypes.c_ulong(); r=ctypes.c_int(); x.XGetInputFocus(d, ctypes.byref(w), ctypes.byref(r)); print(hex(w.value))'; }
echo "x focus before the chord: $(xfocus) (editor $(printf 0x%x "$SHOWN"))"
# Ctrl held, then ';' pressed: T0 is the moment the ';' press reached the X server (XSync), the key event KiCad acts on
T0=$(python3 - <<'PY2'
import ctypes, ctypes.util, os, time
x = ctypes.CDLL(ctypes.util.find_library('X11')); t = ctypes.CDLL(ctypes.util.find_library('Xtst'))
x.XOpenDisplay.restype = ctypes.c_void_p; x.XStringToKeysym.restype = ctypes.c_ulong; x.XKeysymToKeycode.restype = ctypes.c_ubyte
x.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]; t.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]; x.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
d = x.XOpenDisplay(os.environ['DISPLAY'].encode()); kc = lambda n: x.XKeysymToKeycode(d, x.XStringToKeysym(n))
ctrl, semi = kc(b'Control_L'), kc(b'semicolon')
bare = os.environ.get('KC_KEY') == 'semicolon'   # bp.13: KiCad's own ';' (no Ctrl)
if not bare: t.XTestFakeKeyEvent(d, ctrl, 1, 0); x.XSync(d, 0); time.sleep(0.05)
t.XTestFakeKeyEvent(d, semi, 1, 0); x.XSync(d, 0); t0 = int(time.time() * 1000)
time.sleep(0.03); t.XTestFakeKeyEvent(d, semi, 0, 0)
if not bare: t.XTestFakeKeyEvent(d, ctrl, 0, 0)
x.XSync(d, 0)
print(t0)
PY2
)
wait_klog painted || true; OPEN=$(klog open); PAINT=$(klog painted)
OPEN_MS=$(python3 -c 'import json,sys; p=json.loads(sys.argv[1] or "{}"); print(p.get("at_ms",0)-int(sys.argv[2]) if p else -1)' "$PAINT" "$T0")
check "Ctrl+; in KiCad opens Backplane's comment window" "$([ -n "$OPEN" ] && [ -n "$PAINT" ] && echo 1 || echo 0)" "open=$OPEN painted=$PAINT"
shot opened
wait_klog capture || true; CAP=$(klog capture); echo "x focus with the window open: $(xfocus)"
if grep -qa "capture_only" "$DESC"; then check "bp.13: the capture is CommentSelection{capture_only} (no event, no dialog)" "$(python3 -c 'import json,sys; print(1 if json.loads(sys.argv[1] or "{}").get("capture_only") is True else 0)' "$CAP")" "$CAP"; fi
check "the capture names the selection" "$(python3 -c 'import json,sys; c=json.loads(sys.argv[1] or "{}"); print(1 if c.get("ok") and set(c.get("references") or []) >= {"R1","C1"} else 0)' "$CAP")" "$CAP"
SIZES=$(grep -o 'GPUI_RECT {"name":"kcomment"[^}]*}' "$OUT/client.log" | python3 -c 'import json,sys; print(" ".join(sorted({"%dx%d" % (json.loads(l[10:])["w"], json.loads(l[10:])["h"]) for l in sys.stdin})))')
check "the card keeps its size when the capture fills it in" "$([ -n "$SIZES" ] && [ "$(echo $SIZES | wc -w)" = 1 ] && echo 1 || echo 0)" "$SIZES"
python3 "$XINPUT_PY" type "move these closer"; sleep 0.3; shot typed; echo "x focus while typing: $(xfocus)"; python3 "$XINPUT_PY" key Return
wait_klog post || true; wait_klog closed || true; POST=$(klog post)
check "Enter posts the comment and the window shuts" "$([ -n "$POST" ] && [ -n "$(klog closed)" ] && echo 1 || echo 0)" "$POST"
wait_mark posted || true; sleep 1.5; R=$(rep posted)
check "the keyboard is back in KiCad" "$(field "$R" keys | grep -qi true && echo 1 || echo 0)" "keys=$(field "$R" keys)"
# a runtime that hands its comment key over itself (bp.13 batch 4: "comment-key-handoff"): ';' is not reserved, so it types in KiCad's text fields;
# on the canvas and from the "Comment to Backplane..." action (the menu item's) it opens Backplane's window
if grep -qaE "comment-key-(63|handoff)" "$(dirname "$(dirname "$BACKPLANE_KICAD_CLI")")/manifest.json" 2>/dev/null; then
  RK=$(grep "reserved keys" "$OUT/hub.log" | tail -1)
  check "handoff: the bare ';' is not reserved (text fields keep it)" "$(echo "$RK" | grep -q '";"' && echo 0 || echo 1)" "$RK"
  opens() { grep -c '"kcomment":"open"' "$OUT/client.log" || true; }; semis() { grep -c '"kicad_key":";"' "$OUT/client.log" || true; }
  O0=$(opens); S0=$(semis)
  python3 "$XINPUT_PY" chord ctrl g; sleep 1.0; python3 "$XINPUT_PY" type "R1"; python3 "$XINPUT_PY" key semicolon; sleep 1.5; shot search
  check "handoff: ';' typed in KiCad's search box stays there (no key to Backplane, no window)" "$([ "$(opens)" = "$O0" ] && [ "$(semis)" = "$S0" ] && echo 1 || echo 0)" "opens $O0->$(opens) keys $S0->$(semis)"
  python3 "$XINPUT_PY" key Escape; sleep 0.5; click_win_center "$SHOWN"; sleep 0.8
  python3 "$IPC_PY" run-action "$SOCK" common.Control.commentToBackplane >>"$OUT/ipc.err" 2>&1 || true
  for _ in $(seq 60); do [ "$(opens)" -gt "$O0" ] && break; sleep 0.05; done
  check "handoff: the menu item's action opens Backplane's comment window" "$([ "$(opens)" -gt "$O0" ] && echo 1 || echo 0)" "opens $O0->$(opens)"
  python3 "$XINPUT_PY" key Escape; sleep 0.5
fi
# the thread: one message from the person, with the note, the names and the picture (never a second one from the capture's own ET_COMMENT)
MSGS=$(python3 - "$OUT/home/events.jsonl" <<'PY'
import json, sys
n, good = 0, 0
for l in open(sys.argv[1], encoding="utf-8", errors="replace"):
    try: e = json.loads(l)
    except Exception: continue
    if e.get("$") == "MessagePosted" and e.get("role") == "user" and "Selected in the KiCad board" in e.get("text", ""):
        n += 1; t = e["text"]
        good += t.startswith("move these closer") and "R1" in t and "C1" in t and "[attachment: " in t and "kicad-comments" in t
print(f"{n} {good}")
PY
)
check "the thread got one message: the note, the names and the screenshot" "$([ "$MSGS" = "1 1" ] && echo 1 || echo 0)" "messages/good=$MSGS"
IMG=$(ls "$OUT/home/kicad-comments/"*.png 2>/dev/null | head -1)
check "the screenshot is kept beside the hub's data" "$([ -s "$IMG" ] && echo 1 || echo 0)" "$IMG"
DLG=$(grep -c "Comment to Backplane" "$OUT/shots/opened.tree" 2>/dev/null || true)
check "KiCad showed no dialog of its own" "$([ "$DLG" = 0 ] && echo 1 || echo 0)" "dialogs=$DLG"
python3 - "$OUT/client.log" "$T0" <<'PY2'
import json, sys
t0 = int(sys.argv[2]); hops = {}
for l in open(sys.argv[1], errors="replace"):
    if not l.startswith("GPUI_LOG "): continue
    try: v = json.loads(l[9:])
    except Exception: continue
    k = ("received" if v.get("kicad_key") in ("ctrl-;", ";") else "dispatched" if v.get("kicad_key_dispatch") == "ctrl-;" else v.get("kcomment") if v.get("kcomment") in ("open", "opened", "rendered", "painted") else None)
    if k and k not in hops and v.get("at_ms", 0) >= t0: hops[k] = v["at_ms"]
prev = t0; print("hop table (ms after the ';' press reached X; 'painted' is when gpui drew, not pixels on screen):")
for k in ("received", "dispatched", "open", "opened", "rendered", "painted"):
    if k in hops: print(f"  {k:<11} +{hops[k]-t0:>4}  (step {hops[k]-prev})"); prev = hops[k]
PY2
echo "embed_comment_e2e: open_ms=$OPEN_MS (XTest key -> first paint) capture_ms=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("ms"))' "$CAP") hub_ms=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("hub_ms"))' "$CAP")"
wait "$CP" 2>/dev/null || true
finish_suite "embed_comment_e2e ($EMBED_MODE)"
