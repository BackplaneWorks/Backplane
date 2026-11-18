#!/bin/bash
# #1185: the person's own KiCad settings keep their window geometry through an embedded session (fork only).
#   G1  the hub repairs the damage an older runtime's embedded KiCad saved (the issue's Windows numbers and Linux's park position, put
#       into the seeded pcbnew.json / eeschema.json) before the editor starts: the out-of-range entries gone, every other key as it
#       was, a backup kept under <hub home>/backups/kicad-geometry/<version>/, a line in the hub log
#   G2  the embedded board editor opened and closed (Ctrl+W): pcbnew.json's stock window geometry and panel sizes are as before it
#       started. Only a runtime with the fork fix (JSON_SETTINGS::SetKeepWindowGeometry) passes; set GEOM_EXPECT_FORK_FIX=1 for one.
#       Without it G2 is said as INFO, with what the embedded editor wrote (the evidence that every platform has the bug).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_geometry_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
VER=$("$KICAD_CLI" --version 2>/dev/null | tr -d '\r' | tail -1); SV=$(echo "$VER" | cut -d. -f1-2)
D="$XDG_CONFIG_HOME/kicad/$SV"; mkdir -p "$D"
echo "INFO runtime $VER ($KICAD_CLI), settings $D"

# the damage, written into the seeded files (or a minimal one), the rest of each file left as the seed made it
python3 - "$D" <<'PY'
import json, os, sys
d = sys.argv[1]
def load(n):
    p = os.path.join(d, n)
    return json.load(open(p)) if os.path.exists(p) else {"meta": {"filename": n, "version": 0}}
pcb = load("pcbnew.json")
w = pcb.setdefault("window", {})
w.update({"pos_x": -1137246128, "pos_y": 10, "size_x": 2071176320, "size_y": 32245, "maximized": False})
pcb.setdefault("aui", {}).update({"right_panel_width": 1239146100, "properties_panel_width": 1239146100})
pcb.setdefault("other_person_key", {"kept": [1, 2.50, "x"]})
json.dump(pcb, open(os.path.join(d, "pcbnew.json"), "w"), indent=2); open(os.path.join(d, "pcbnew.json"), "a").write("\n")
sch = load("eeschema.json")
sch.setdefault("window", {}).update({"pos_x": -30000, "pos_y": -30000, "size_x": 1395, "size_y": 1242, "maximized": True})
json.dump(sch, open(os.path.join(d, "eeschema.json"), "w"), indent=2); open(os.path.join(d, "eeschema.json"), "a").write("\n")
PY
cp "$D/pcbnew.json" "$OUT/pcbnew.damaged.json"; cp "$D/eeschema.json" "$OUT/eeschema.damaged.json"

start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":600000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 700 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; exit 1; }
cp "$D/pcbnew.json" "$OUT/pcbnew.repaired.json"; cp "$D/eeschema.json" "$OUT/eeschema.repaired.json"
grep -a "window geometry (#1185)" "$OUT/hub.log" | sed 's/^/INFO hub: /' || true

# G1: the repair, measured on the files as they were when the editor came up
G1=$(python3 - "$OUT" "$OUT/home/backups/kicad-geometry/$SV" <<'PY'
import json, os, sys
out, bk = sys.argv[1:]
L = lambda p: json.load(open(p))
def big(v): return isinstance(v, (int, float)) and not isinstance(v, bool) and abs(v) >= 20000
def strip(doc, gone):
    for path in gone:
        o = doc
        *head, last = path.split(".")
        for k in head: o = o[k]
        o.pop(last, None)
    return doc
why = []
for n, gone in (("pcbnew", ["window.pos_x", "window.size_x", "window.size_y", "aui.right_panel_width", "aui.properties_panel_width"]),
                ("eeschema", ["window.pos_x", "window.pos_y"])):
    dmg, now = L(f"{out}/{n}.damaged.json"), L(f"{out}/{n}.repaired.json")
    if strip(dmg, gone) != now: why.append(f"{n}: not exactly the out-of-range entries removed")
    if not os.path.exists(f"{bk}/{n}.json") or open(f"{bk}/{n}.json").read() != open(f"{out}/{n}.damaged.json").read(): why.append(f"{n}: no backup of the damaged file")
print("1 ok" if not why else "0 " + "; ".join(why))
PY
)
check "G1 the hub removed only the out-of-range geometry before the editor started, and kept a backup" "${G1%% *}" "${G1#* }"

# G2: the embedded editor opened, then closed by Ctrl+W (KiCad saves its settings as a frame closes)
pcb_frame() { python3 "$DRIVER_PY" list-windows --match "PCB Editor" 2>/dev/null | python3 -c 'import json,sys; ws=[w for w in json.load(sys.stdin) if w.get("shown") and w["w"]>300]; print(ws[0]["xid"], ws[0]["root_x"], ws[0]["root_y"], ws[0]["w"], ws[0]["h"]) if ws else print("")'; }
read -r F FX FY FW FH <<<"$(pcb_frame)"
[ -n "${F:-}" ] && python3 "$XINPUT_PY" click $((FX + FW / 2)) $((FY + FH - 120)); sleep 0.8
python3 "$XINPUT_PY" chord ctrl w
for _ in $(seq 40); do [ -z "$(pcb_frame)" ] && break; sleep 0.25; done; sleep 2
cp "$D/pcbnew.json" "$OUT/pcbnew.after.json"
G2=$(python3 - "$OUT" <<'PY'
import json, sys
out = sys.argv[1]
a, b = json.load(open(f"{out}/pcbnew.repaired.json")), json.load(open(f"{out}/pcbnew.after.json"))
keys = lambda d: {**{f"window.{k}": d.get("window", {}).get(k) for k in ("pos_x", "pos_y", "size_x", "size_y", "maximized", "display", "perspective", "aui_state")},
                  **{f"aui.{k}": v for k, v in d.get("aui", {}).items() if k.endswith(("_width", "_height", "_size", "_pos", "_proportion"))}}
ka, kb = keys(a), keys(b)
diff = {k: (ka.get(k), kb.get(k)) for k in set(ka) | set(kb) if ka.get(k) != kb.get(k)}
print("1 unchanged" if not diff else "0 " + ", ".join(f"{k}: {x} -> {str(y)[:40]}" for k, (x, y) in sorted(diff.items())))
PY
)
if [ "${GEOM_EXPECT_FORK_FIX:-0}" = 1 ]; then
  check "G2 the embedded editor's close left pcbnew.json's stock window geometry and panel sizes as they were" "${G2%% *}" "${G2#* }"
else
  echo "INFO G2 (runtime $VER, no fork fix expected): the embedded editor's close wrote: ${G2#* }"
fi
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_geometry_e2e ($EMBED_MODE, $VER)"
