#!/bin/bash
# A design tab pressed from another open page (Board <-> Schematic, 3D, Gerbers, Files) shows its own page loading at once, never the page it
# left under its lit tab (a 0.16 report: the tab lit at the press, its content came seconds later, so the person kept clicking). Cold and
# warm switches, and a run of rapid presses (only the last one counts: an earlier page's arrival never hides the last tab's loading).
# Read from the window itself ({"read":"pane"}: what the pane drew last and
# the rail's lit kind) every 100 ms, with a shot just after each press.
#   EMBED_MODE=fork test/kicad_embed/tab_loading_e2e.sh                         Xvfb (a build box)
#   KEMBED_DISPLAY=:0 EMBED_MODE=fork test/kicad_embed/tab_loading_e2e.sh       the person's desktop: an isolated hub and client, no XTest
# KEMBED_PROJ_SRC=<project folder> for a larger design (a slower cold plot); the seeded fixture otherwise. KEMBED_HUB_DELAY_MS=<ms>: the client talks
# to the hub through lib/slow_proxy.py, which holds each of the hub's answers that long (a busy hub: the report's case).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
if [ -n "${KEMBED_DISPLAY:-}" ]; then DISP="$KEMBED_DISPLAY"; export DISPLAY="$KEMBED_DISPLAY" GDK_BACKEND=x11 GDK_SCALE=1; else start_xvfb_and_wm; fi
start_hub
CPORT=$HPORT
if [ "${KEMBED_HUB_DELAY_MS:-0}" -gt 0 ]; then
  CPORT=$(free_port); own python3 -I "$KEMBED_LIB/slow_proxy.py" "$CPORT" "$HPORT" "$KEMBED_HUB_DELAY_MS"
  for _ in $(seq 50); do curl -s -m 5 -o /dev/null "http://127.0.0.1:$CPORT/hello" && break; sleep 0.1; done
  echo "INFO the hub's answers held ${KEMBED_HUB_DELAY_MS} ms (port $CPORT -> $HPORT)"
fi
python3 - "$OUT/script.json" "${KEMBED_HUB_DELAY_MS:-0}" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2500 + 3 * int(sys.argv[2])}]
def phase(name, presses, reads, gap=60):
    s.append({"mark": name})
    for i, p in enumerate(presses):
        if i: s.append({"wait": gap})
        s.append({"press": "rail:" + p})
    s.append({"mark": name + ":pressed"})
    for _ in range(reads): s.extend([{"read": "pane"}, {"wait": 100}])
phase("cold-board", ["Board"], 150)
phase("cold-schematic", ["Schematic"], 150)
phase("warm-board", ["Board"], 80)
phase("warm-schematic", ["Schematic"], 80)
phase("cold-3d", ["3D"], 100)
phase("cold-gerbers", ["Gerbers"], 100)
phase("rapid", ["Board", "3D", "Schematic", "Files", "Board"], 60)
phase("files", ["Files"], 80)
s += [{"mark": "done"}, {"wait": 500}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$CPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the client's own window on a desktop (by its pid); the whole screen on Xvfb
win() { for w in $(xwininfo -root -tree | awk '/Backplane/ {print $1}'); do [ "$(xprop -id "$w" _NET_WM_PID 2>/dev/null | awk '{print $3}')" = "$CP" ] && { echo "$w"; return; }; done; }
snap() { if [ -n "${KEMBED_DISPLAY:-}" ]; then local w; w=$(win); [ -n "$w" ] && import -window "$w" "$OUT/shots/$1.png" 2>/dev/null || true; else shot "$1"; fi; }
for m in cold-board cold-schematic warm-board warm-schematic cold-3d cold-gerbers rapid files done; do
  pat="\"name\":\"$m:pressed\""; [ "$m" = done ] && pat='"name":"done"'
  for _ in $(seq 4800); do grep -q "$pat" "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.05; done
  [ "$m" = done ] || { snap "$m-0"; sleep 1.2; snap "$m-1"; }
done
wait "$CP" 2>/dev/null || true
python3 -I - "$OUT/client.log" > "$OUT/verdict.txt" <<'PY' || true
import json, sys
want = {"cold-board": "board", "cold-schematic": "schematic", "warm-board": "board", "warm-schematic": "schematic", "cold-3d": "3d", "cold-gerbers": "gerbers",
        "rapid": "board", "files": "files"}
ev, cur, t0 = {}, None, {}
for line in open(sys.argv[1], errors="replace"):
    if "GPUI_METRIC " not in line: continue
    m = json.loads(line.split("GPUI_METRIC ", 1)[1])
    if m.get("event") == "mark":
        n = m["name"]
        if n.endswith(":pressed"): cur = n[:-8]; t0[cur] = m["t_ms"]; ev[cur] = []
        else: cur = None
    elif m.get("event") == "read" and m.get("what") == "pane" and cur:
        page, say, lit, err = (m.get("text", "") + "|||").split("|")[:4]
        ev[cur].append((m["t_ms"] - t0[cur], page, say, lit, err))
for name, k in want.items():
    r = ev.get(name, [])
    if not r: print(f"FAIL\t{name}: no reads"); continue
    kind = lambda p: p.split(" ")[0]
    # the shown page's own kind: Files may land on a file; Gerbers is the Board page drawing a set
    same = lambda p: kind(p) == k or (k == "files" and kind(p) in ("image", "pdf", "text")) or (k == "gerbers" and kind(p) == "board")
    pressed = lambda x: same(x[1]) and x[3] == k
    # the hub said no (a refusal that came after the press: an earlier one's line may still be on screen): from then the tab gives way to
    # the page on show, the window saying why (the line goes after a while)
    refusal = next((i for i, x in enumerate(r) if "refused" in x[4] and i > 0 and "refused" not in r[i - 1][4]), None)
    gave_way = lambda x: x[3] == kind(x[1]) != k
    # a read just after the press, or just as a refusal lands, may come before that frame is drawn
    wrong = [x for i, x in enumerate(r) if x[0] > 150 and i != refusal and not (pressed(x) or (refusal is not None and i > refusal and gave_way(x)))]
    first = r[0]
    print(("PASS" if not wrong else "FAIL") + f"\t{name}: every read shows the pressed tab lit with its own page, or the hub's refusal ({len(r)} reads; first {first[0]:.0f} ms: {first[1]!r} {first[2]!r} lit {first[3]!r}"
          + (f"; wrong {len(wrong)}, first {wrong[0]}" if wrong else "") + ")")
    if refusal is not None:
        x = r[refusal]; print(("PASS" if all(gave_way(y) for y in r[refusal + 1:]) else "FAIL") + f"\t{name}: refused at {x[0]:.0f} ms ({x[4]!r}): the tab gives way and stays so")
        continue
    ready = next((x[0] for x in r if same(x[1]) and " pressed" not in x[1] and not x[2]), None)
    loading = [x for x in r if x[2] or " pressed" in x[1]]
    print(("PASS" if ready is not None else "FAIL") + f"\t{name}: its page shown, the loading line gone" + (f" at {ready:.0f} ms ({len(loading)} reads loading)" if ready is not None else " (never, in the reads)"))
    after = [x for x in r if ready is not None and x[0] > ready and not same(x[1])]
    blips = [x for x in r if ready is not None and x[0] > ready and same(x[1]) and x[2]]
    print(("PASS" if not after else "FAIL") + f"\tname: once shown no other page replaces it".replace("name", name) + (f" (then {after[0]})" if after else ""))
    if blips: print(f"KNOWN\t{name}: its own loading line came back {len(blips)} reads after it was shown (a late frame of a page pressed on the way: the plot store holds one frame, main does the same)")
PY
while IFS=$'\t' read -r v what; do if [ "$v" = KNOWN ]; then echo "KNOWN $what"; else check "$what" "$([ "$v" = PASS ] && echo 1 || echo 0)" ""; fi; done < "$OUT/verdict.txt"
[ -s "$OUT/verdict.txt" ] || check "the window's reads were parsed" 0 "see $OUT/client.log"
echo "shots: $OUT/shots"
finish_suite "tab_loading_e2e ($EMBED_MODE)"
