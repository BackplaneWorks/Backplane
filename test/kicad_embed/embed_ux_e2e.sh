#!/bin/bash
# Suite 4: the editor's own row (stock|fork): a tab per editor (Board, Schematic, Footprints), each keeping its frame (same window, same
# KiCad), Full size (the pane takes the sidebar's and the thread's room, and gives it back), Pop out (the same KiCad in a window of its
# own) and Pop back in (the pane again, nothing restarted).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "spike" ] && { echo "embed_ux_e2e: stock|fork only"; exit 0; }
discover_bins
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
# Each step waits on the editor's state (wait_painted: KiCad ready and the wanted editor shown and painted), not a fixed time, so a slow
# machine passes; how long each took is a separate timing check against its budget (KEMBED_TIME_SCALE multiplies them on a slow venue).
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}]
def rep(m, kind, t=120000, w=1500): return [{"kicad": {"wait_painted": t, "kind": kind, "tag": m}}, {"mark": m}, {"wait": w}]
s += rep("pcb", "PCB")
s += [{"click": "kicad-tab:SCH"}] + rep("sch", "SCH")
s += [{"click": "kicad-tab:FOOTPRINT_EDITOR"}] + rep("fp", "FOOTPRINT_EDITOR")
s += [{"click": "kicad-tab:PCB"}] + rep("pcb2", "PCB")
s += [{"click": "kicad:full"}, {"wait": 1500}] + rep("full", "PCB")
s += [{"click": "kicad:full"}, {"wait": 1500}] + rep("unfull", "PCB")
s += [{"click": "kicad:popout"}, {"wait": 1000}] + rep("popped", "PCB", w=2000)
s += [{"click": "kicad:popin"}, {"wait": 1000}] + rep("back", "PCB")
s += [{"click": "ui:View"}, {"wait": 1500}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 900 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
seen=""
while kill -0 "$CP" 2>/dev/null; do
  for m in $(grep -o '"name":"[a-z_0-9]*","event":"mark"' "$OUT/client.log" 2>/dev/null | cut -d'"' -f4); do
    case " $seen " in *" $m "*) continue ;; esac; seen="$seen $m"; sleep 1.2; shot "$m"
  done
  sleep 0.2
done
python3 - "$OUT" "$EMBED_MODE" <<'PY'
import json, os, sys
out, mode = sys.argv[1], sys.argv[2]
reps = {}
for l in open(os.path.join(out, "client.log"), errors="ignore"):
    if '"event":"kicad"' in l:
        r = json.loads(l[l.index("{"):])
        if r.get("tag"): reps.setdefault(r["tag"], r)
def shown(r):
    for f in r.get("frames", []):
        if f.get("xid") == r.get("shown"): return f
    return {}
ok = fail = 0
def check(what, cond, detail=""):
    global ok, fail
    cond = bool(cond)
    print(("PASS " if cond else "FAIL ") + what + ("" if cond else f" ({str(detail)[:300]})")); ok += cond; fail += (not cond)
p, s, fp, p2, fu, un, po, bk = (reps.get(k, {}) for k in ("pcb", "sch", "fp", "pcb2", "full", "unfull", "popped", "back"))
check("Board tab: the PCB editor shown and painted", shown(p).get("kind") == "PCB" and shown(p).get("painted", 0) > 0, shown(p))
check("Schematic tab: the schematic editor shown and painted", shown(s).get("kind") == "SCH" and shown(s).get("painted", 0) > 0, shown(s))
check("Footprints tab: the footprint editor opened in the pane", shown(fp).get("kind") == "FOOTPRINT_EDITOR" and shown(fp).get("painted", 0) > 0, shown(fp))
check("Board tab again: the same PCB frame (its state kept), same KiCad", p2.get("shown") == p.get("shown") and p2.get("pid") == p.get("pid") and p.get("pid"), (p.get("shown"), p2.get("shown"), p.get("pid"), p2.get("pid")))
cw = lambda r: (r.get("container") or {}).get("w", 0)
check("Full size: the pane takes the sidebar's and the thread's room", cw(fu) > cw(p2) + 300 and shown(fu).get("w", 0) >= cw(fu) - 4, (cw(p2), cw(fu)))
check("Full size off: the pane as it was", abs(cw(un) - cw(p2)) <= 4, (cw(p2), cw(un)))
check("Pop out: the same KiCad in a window of its own, painted", po.get("toplevel") and po.get("toplevel") != p.get("toplevel") and po.get("pid") == p.get("pid") and shown(po).get("painted", 0) > 0, (p.get("toplevel"), po.get("toplevel"), po.get("pid"), shown(po)))
check("Pop back in: the main window's pane again, the same frame and KiCad", bk.get("toplevel") == p.get("toplevel") and bk.get("pid") == p.get("pid") and bk.get("shown") == p.get("shown") and shown(bk).get("painted", 0) > 0, (bk.get("toplevel"), bk.get("shown"), shown(bk)))
# timing budgets, separate from the checks above (a slow venue scales them with KEMBED_TIME_SCALE; real-hardware numbers: real_use_*.sh)
scale = float(os.environ.get("KEMBED_TIME_SCALE", "1"))
for tag, what, lim in (("pcb", "Edit to the Board painted", 15000), ("sch", "Schematic tab painted", 5000), ("fp", "Footprints tab painted", 10000),
                       ("pcb2", "Board tab again painted", 3000), ("back", "Pop back in painted", 5000)):
    ms = reps.get(tag, {}).get("wait_ms")
    print(f"PERF ux_{tag} {ms}ms")
    check(f"time: {what} within {int(lim * scale)} ms", ms is not None and ms <= lim * scale, ms)
print(f"embed_ux_e2e ({mode}): passed={ok} failed={fail} artifacts={out}")
sys.exit(1 if fail else 0)
PY
