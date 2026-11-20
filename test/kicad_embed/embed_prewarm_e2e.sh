#!/bin/bash
# A pre-warm that KiCad stops with a dialog of its own (a schematic it repairs on loading: test/files/fixtures/broken-toplevel, the
# project file's top-level sheet UUID not the schematic's) never leaves the pane on a silent "Starting KiCad…": the Schematic tab shows the
# editor, or says which dialog KiCad has open. A KiCad that raises no dialog (bp.12 with the fork's fix) must also leave the pre-warmed
# schematic unmodified. Every check counts only when the schematic was loaded hidden before the tab was pressed: its hidden frame in the
# report taken just before the press (loaded by editor.kicad.prewarm, or on bp.13.2 by the hub's checks' DRC parity), or the pre-warm's
# reply building SCH before the press; with neither, the Schematic tab loads it itself and the checks would pass on nothing.
# KEMBED_FORCE_REFUSE=prewarm refuses the pre-warm (with BACKPLANE_CHECKS_IN_KICAD=0 nothing else loads it, and the checks fail). fork;
# KEMBED_PROJ_SRC for another project.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_prewarm_e2e: fork only"; exit 0; }
export KEMBED_PROJ_SRC=${KEMBED_PROJ_SRC:-$REPO_ROOT/test/files/fixtures/broken-toplevel}
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
[ "${KEMBED_FORCE_REFUSE:-}" = prewarm ] && export BACKPLANE_KICAD_PREWARM_FREE_MB=10000000
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "edit"}},
     {"wait": 7000}, {"kicad": {"report": True, "tag": "prewarmed"}}, {"click": "kicad-tab:SCH"}, {"wait": 4000}, {"kicad": {"report": True, "tag": "sch"}}, {"mark": "done"}, {"wait": 4000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 60 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 900); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" sch)
shot sch
IFS=$'\t' read -r SOCK SAY DLG PAINTED < <(python3 -c '
import json, sys
r = json.loads(sys.argv[1] or "{}")
f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown") and f.get("kind") == "SCH"), {})
d = r.get("dialogs") or []
print(r.get("sock") or "-", json.dumps(r.get("say") or "", ensure_ascii=False), json.dumps(d[-1] if d else "", ensure_ascii=False), f.get("painted", 0), sep="\t")' "$R")
echo "INFO say=$SAY dialog=$DLG sch_painted=$PAINTED"
# the schematic was loaded hidden before the tab was pressed: its hidden frame in the report taken just before the press, or the pre-warm
# answered building it before the press
IFS=$'\t' read -r PREWARMED PREWARM_SAY < <(python3 - "$OUT/client.log" "$OUT/hub.log" "$(last_kicad_report "$OUT/client.log" prewarmed)" <<'PY'
import json, sys
client, hub, rep = sys.argv[1], sys.argv[2], json.loads(sys.argv[3] or "{}")
reply = None; press = None
for l in open(client, errors="ignore"):
    if '"id":"kicad-tab:SCH","changed"' in l and '"event":"click"' in l and press is None:   # the press, not the controls/rects inventory
        try: press = json.loads(l.split("GPUI_METRIC ", 1)[1]).get("epoch_ms")
        except Exception: pass
    if "GPUI_LOG " not in l: continue
    try: o = json.loads(l.split("GPUI_LOG ", 1)[1])
    except Exception: continue
    if o.get("kicad_rpc_reply") == "editor.kicad.prewarm": reply = reply or o
h = open(hub, errors="ignore").read()
refused = [l for l in h.splitlines() if "not pre-warming" in l]
# loaded hidden before the tab is pressed: its frame there and not shown in the report taken just before the press, or (a dialog of its
# load holding the frame back) the client's pre-warm answered building SCH before the press. The checks' line "parity loads the schematic
# hidden" is the hub's one-time capability line (it can, not that it did): wording only, never evidence
sch = any(f.get("kind") == "SCH" and not f.get("shown") for f in rep.get("frames", []))
asked = bool(reply) and "SCH" in (reply.get("prewarming") or []) and press is not None and (reply.get("at_ms") or 0) < press
by = [w for w, yes in (("the pre-warm", asked), ("the checks' DRC parity (can: capability line)", "parity loads the schematic hidden" in h)) if yes]
ok = sch or asked
print(int(ok), " ".join(map(str, ["SCH hidden:", sch, "by:", " or ".join(by) or "?", "pre-warm reply:", reply and reply.get("prewarming"), "|", refused[0] if refused else ""]))[:300], sep="\t")
PY
)
check "the schematic was loaded hidden before the tab was pressed" "$PREWARMED" "$PREWARM_SAY"
SURFACED=0
if [ "${PAINTED:-0}" -ge 15 ]; then SURFACED=1
elif [ "$DLG" != '""' ] && python3 -c 'import json,sys; s,d=json.loads(sys.argv[1]),json.loads(sys.argv[2]); sys.exit(0 if d and d in s else 1)' "$SAY" "$DLG"; then SURFACED=1; fi
check "the Schematic tab shows the editor or names KiCad's open dialog, after a pre-warm" "$([ "$PREWARMED" = 1 ] && echo "$SURFACED" || echo 0)" "prewarmed $PREWARMED, say=$SAY dialog=$DLG"
check "never a silent Starting KiCad once KiCad is up" "$(echo "$SAY" | grep -q 'Starting' && echo 0 || echo 1)" "say=$SAY"
if [ "$DLG" = '""' ]; then
  MOD=$(python3 "$IPC_PY" modified "$SOCK" sch 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?")
  check "the pre-warmed schematic is not modified" "$([ "$PREWARMED" = 1 ] && [ "$MOD" = False ] && echo 1 || echo 0)" "prewarmed $PREWARMED, modified=$MOD"
else
  echo "KNOWN this KiCad raises a modal on the repaired schematic's load (fork fix 8e57d691cd, bp.12): the modified check needs it gone"
fi
wait "$CP" 2>/dev/null || true
finish_suite "embed_prewarm_e2e ($EMBED_MODE)"
