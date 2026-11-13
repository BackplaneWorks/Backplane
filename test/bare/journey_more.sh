# Sourced by journey_project.sh after the agent's co-edit (KiCad open on pic_programmer): the management panels and the tools that landed
# since the gate began. Each step has its own check; a missing control or capability is a FAIL that says which.
#   BOM (Alt+Shift+B), Checks + a cross-probe click (GetSelection over KiCad's IPC), Fab + Review (.backplane/design-review.json),
#   Ctrl+; comment window (a message in the thread), Gerbers tab on mfg/ exported with the runtime's kicad-cli, and the floorplanner's
#   plan tools through the stand-in agent (propose -> route refused -> the person approves -> route).
PRO=$PROJ/pic_programmer.kicad_pro; HPRO=$HH/Documents/pic_programmer/pic_programmer.kicad_pro
ctl_xy() { python3 - "$HH/win.log" "$1" <<'PY'
import json, re, sys
f = None
for l in open(sys.argv[1], errors="replace"):
    if "GPUI_METRIC" not in l or '"controls"' not in l: continue
    try: v = json.loads(l[l.index("{"):])
    except Exception: continue
    for c in v.get("controls", []):
        if isinstance(c, dict) and re.fullmatch(sys.argv[2], c.get("id", "")) and c.get("rect") and c.get("visible", True): f = c
if f: print(int(f["rect"]["x"] + min(f["rect"]["w"] / 2, 60)), int(f["rect"]["y"] + f["rect"]["h"] / 2), f["id"])
PY
}
klog() { grep -h "GPUI_LOG" "$HH/win.log" | grep "\"kcomment\":\"$1\"" | tail -1; }

# Gerbers for the Gerbers tab, as a person exports them (the runtime's kicad-cli into the project's mfg/)
as_person sh -c "mkdir -p $PROJ/mfg && $KB/bin/kicad-cli pcb export gerbers -o $PROJ/mfg/ $PCB && $KB/bin/kicad-cli pcb export drill -o $PROJ/mfg/ $PCB" >"$OUT/logs/gerbers-export.log" 2>&1
[ "$(ls "$HH/Documents/pic_programmer/mfg" 2>/dev/null | wc -l)" -gt 3 ] && ok "gerbers exported to mfg/ ($(ls "$HH/Documents/pic_programmer/mfg" | wc -l) files)" || bad "gerber export failed: $(tail -2 "$OUT/logs/gerbers-export.log")"

WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
cat >"$HH/panels.json" <<J
[{"wait":3000},{"click":"project:$pid_"},{"wait":2000},{"click":"page-thread:$tid"},{"wait":2500},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"p2"}},{"mark":"p2-ready"},{"wait":1500},
 {"keys":"alt-shift-b"},{"wait":3000},{"mark":"bom"},{"click":"mgmt:checks"},{"wait":8000},{"mark":"checks"},
 {"wait":240000},{"mark":"end2"}]
J
: >"$HH/win.log"; T2=$(date +%s.%N); launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/panels.json
since2() { python3 -c "import time;print(round(time.time()-$T2,1))"; }
markt p2-ready 400 || { bad "the second session never showed KiCad again"; return 0; }
markt bom 60; sleep 1; shot 11-bom
BOM=$(hub rpc mgmt.watch "{\"project\":\"$PRO\"}" | python3 -c 'import json,sys
r=json.load(sys.stdin); d=((r.get("result") or {}).get("data") or {}); b=d.get("bom") or {}; s=(r.get("result") or {}).get("state") or {}
print(json.dumps({"open": s.get("open"), "lines": len(b.get("lines") or []), "parts": b.get("parts"), "source": b.get("source")}))' 2>/dev/null)
echo "bom: $BOM"
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); sys.exit(0 if d.get("open")=="bom" and d.get("lines",0)>0 else 1)' "$BOM" 2>/dev/null \
  && { ok "Alt+Shift+B opens the BOM panel with the board's lines ($BOM)"; tm bom-panel "$(since2)"; } || { bad "Alt+Shift+B: no BOM panel with lines ($BOM)"; cut_ 2 bom "Alt+Shift+B did not open a BOM with lines on a fresh install: $BOM"; }

# Checks, then a click on a DRC row: its items are selected in the PCB editor (GetSelection over KiCad's own socket, mounted from the box)
markt checks 60; sleep 2; shot 12-checks
SOCK=$(report p2 | python3 -c 'import json,sys
l=sys.stdin.read(); print(json.loads(l[l.index("{"):]).get("sock","") if "{" in l else "")' 2>/dev/null); HSOCK=${SOCK/#$PH/$HH}
CHK=$(hub rpc mgmt.watch "{\"project\":\"$PRO\"}" | python3 -c 'import json,sys
r=json.load(sys.stdin); c=(((r.get("result") or {}).get("data") or {}).get("checks") or {})
rows=[x for g in c.get("groups",[]) for x in g.get("rows",[])[:3]]
d=[x for x in rows if x.get("source")=="drc"]
print(json.dumps({"drc": c.get("drc",{}).get("state"), "erc": c.get("erc",{}).get("state"), "rows": len(rows), "row": (d[0]["id"] if d else "")}))' 2>/dev/null)
echo "checks: $CHK"
ROW=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("row",""))' "$CHK" 2>/dev/null)
if [ -n "$ROW" ]; then
  read -r cx cy cid < <(ctl_xy "mgmt:checks:row:$ROW")
  if [ -n "${cx:-}" ]; then
    S0=$(python3 "$HERE/../kicad_embed/lib/gui_ipc_probe.py" selection "$HSOCK" pcb 2>&1 | tail -1)
    t=$(date +%s%N); X mousemove "$cx" "$cy" click 1; sleep 1.5
    S1=$(python3 "$HERE/../kicad_embed/lib/gui_ipc_probe.py" selection "$HSOCK" pcb 2>&1 | tail -1); shot 13-crossprobe
    echo "selection before: $S0 after: $S1" >"$OUT/logs/crossprobe.txt"
    [ "$S1" != "$S0" ] && echo "$S1" | grep -q '"ok": *true\|"types"' && ok "a click on a DRC row selects its items in the PCB editor ($(echo "$S1" | cut -c1-120))" \
      || { bad "the DRC row click did not change KiCad's selection ($S0 -> $S1)"; cut_ 2 checks "a Checks row click did not cross-probe into KiCad: $(echo "$S1" | cut -c1-160)"; }
  else bad "the Checks row mgmt:checks:row:$ROW is not drawn"; fi
else bad "the Checks panel has no DRC row ($CHK)"; fi

# Fab, then Review: the design review writes .backplane/design-review.json in the project
read -r fx fy _ < <(ctl_xy "mgmt:fab"); [ -n "${fx:-}" ] && X mousemove "$fx" "$fy" click 1; sleep 3; shot 14-fab
read -r rx ry _ < <(ctl_xy "mgmt:fab:review")
if [ -n "${rx:-}" ]; then t=$(date +%s.%N); X mousemove "$rx" "$ry" click 1
  for _ in $(seq 240); do [ -s "$HH/Documents/pic_programmer/.backplane/design-review.json" ] && break; sleep 0.5; done; shot 15-review
  [ -s "$HH/Documents/pic_programmer/.backplane/design-review.json" ] && { ok "Fab, Review: the design review wrote .backplane/design-review.json ($(python3 -c "import json;d=json.load(open('$HH/Documents/pic_programmer/.backplane/design-review.json'));print(len(d.get('findings',[])),'findings')" 2>/dev/null))"; tm review "$(python3 -c "import time;print(round(time.time()-$t,1))")"; } \
    || { bad "Review wrote no design-review.json in 120 s"; cut_ 2 review "the Fab panel's Review wrote nothing on a fresh install"; }
else bad "no Fab panel or Review button (mgmt:fab, mgmt:fab:review)"; fi

# Ctrl+; in KiCad: Backplane's comment window; a note and Enter post one message to the thread
read -r px py pw ph < <(report p2 | python3 -c 'import json,sys
l=sys.stdin.read(); c=json.loads(l[l.index("{"):]).get("container",{}) if "{" in l else {}
print(c.get("root_x",0)+c.get("w",0)//2, c.get("root_y",0)+c.get("h",0)//2, c.get("w",0), c.get("h",0))')
X mousemove "$px" "$py" click 1; sleep 0.8; X key Escape; sleep 0.3; X key ctrl+semicolon
for _ in $(seq 40); do [ -n "$(klog painted)" ] && break; sleep 0.25; done; shot 16-comment
if [ -n "$(klog painted)" ]; then ok "Ctrl+; in KiCad opens Backplane's comment window"
  X type --delay 30 "bare gate note"; sleep 0.3; X key Return
  for _ in $(seq 40); do grep -q '"role":"user"[^}]*bare gate note\|bare gate note[^}]*"role":"user"' "$HUBH/events.jsonl" 2>/dev/null && break; sleep 0.25; done
  grep -q 'bare gate note' "$HUBH/events.jsonl" && ok "the comment posted to the thread" || bad "the comment window posted nothing to the thread"
else bad "Ctrl+; opened no comment window"; cut_ 2 comment "Ctrl+; in the embedded KiCad opened no comment window on a fresh install"; fi

# Gerbers tab: the mfg/ set drawn
read -r gx gy _ < <(ctl_xy "rail:Gerbers"); [ -n "${gx:-}" ] || { p=$(pos rail:Gerbers) && read -r gx gy <<<"$p"; }
if [ -n "${gx:-}" ]; then t=$(date +%s.%N); X mousemove "$gx" "$gy" click 1
  for _ in $(seq 60); do grep -q '"event":"plot_painted".*mfg\|mfg.*"event":"plot_painted"' "$HH/win.log" && break; sleep 0.5; done; sleep 1; shot 17-gerbers
  grep -q '"event":"plot_painted".*mfg\|mfg.*"event":"plot_painted"' "$HH/win.log" && { ok "the Gerbers tab drew the project's mfg/ set"; tm gerbers "$(python3 -c "import time;print(round(time.time()-$t,1))")"; } \
    || { bad "the Gerbers tab drew no mfg/ set"; cut_ 2 gerbers "the Gerbers tab did not draw the exported mfg/ set"; }
else bad "no Gerbers tab on the rail"; fi

# The floorplanner's plan tools, through the stand-in agent: propose, a refused route, the person's approval, the route. They need the
# runtime's `routing-plan` capability: a runtime without it is a paper cut (what setup installs cannot do it), not a crash or a FAIL.
CAPS=$(python3 -c "import json;print(' '.join(json.load(open('$KDIR/manifest.json')).get('capabilities',[])))" 2>/dev/null)
if ! printf ' %s ' "$CAPS" | grep -q ' routing-plan '; then
  cut_ 2 plan "setup installs Backplane KiCad $(basename "$KDIR"), which has no routing-plan$(printf ' %s ' "$CAPS" | grep -q ' coedit ' || echo ' and no coedit'): the floorplanner$(printf ' %s ' "$CAPS" | grep -q ' coedit ' || echo ' and per-agent undo') cannot work on a fresh install"
  printf 'SKIP\tplan tools: the runtime has no routing-plan (%s)\n' "$(basename "$KDIR")" >>"$OUT/checks.tsv"; echo "SKIP plan tools (no routing-plan in $(basename "$KDIR"))"
else
python3 - "$HPRO" "$HH/standin.conf.plan" <<'PY'
import json, re, sys
pcb = sys.argv[1].replace(".kicad_pro", ".kicad_pcb"); s = open(pcb).read()
fps = []
for m in re.finditer(r'\(footprint "[^"]*"', s):
    end = s.find('\n\t(footprint', m.end()); blk = s[m.start(): end if end > 0 else len(s)]
    ref = re.search(r'\(property "Reference" "([^"]+)"', blk) or re.search(r'\(fp_text reference "([^"]+)"', blk)
    at = re.search(r'\n\t\t\(at ([-\d.]+) ([-\d.]+)', blk); nets = re.findall(r'\(net (?:\d+ )?"([^"]+)"\)', blk)
    nets = [n for n in nets if n and n not in ("GND", "VCC") and not n.startswith("unconnected")]
    if ref and at and nets: fps.append((ref.group(1), float(at.group(1)), float(at.group(2)), nets[0]))
a, b = fps[0], next(f for f in fps[1:] if f[3] != fps[0][3])
plan = {"layers": [{"layer": "B.Cu", "role": "signal", "status": "proposed"}],
        "groups": [{"id": "g-bare", "name": "Bare gate", "color": "#e8a33d", "nets": [a[3], b[3]], "status": "proposed"}],
        "corridors": [{"id": "c-bare", "group": "g-bare", "layer": "B.Cu", "order": 1, "path": [[a[1], a[2]], [b[1], b[2]]], "width": 2.0,
                       "from": [a[0]], "to": [b[0]], "status": "proposed"}]}
json.dump(plan, open(sys.argv[2], "w"))
PY
python3 - "$OUT/standins/bin/coedit.conf" "$HH/standin.conf.plan" <<'PY'
import json, sys
c = json.load(open(sys.argv[1])); c["plan"] = json.load(open(sys.argv[2])); json.dump(c, open(sys.argv[1], "w"))
PY
P1=$HH/coedit-plan1.json; P2=$HH/coedit-plan2.json; rm -f "$P1" "$P2"
hub rpc turn.start "{\"thread\":\"$tid\",\"text\":\"plan-propose\"}" >/dev/null
for _ in $(seq 240); do [ -s "$P1" ] && break; sleep 0.25; done
if [ -s "$P1" ]; then cp "$P1" "$OUT/logs/"
  python3 - "$P1" >"$OUT/logs/plan1-checks.txt" <<'PY'
import json, sys
r = json.load(open(sys.argv[1])); t = lambda k: json.dumps(r.get(k, {}))
print("propose", "ok" if not r.get("propose", {}).get("error") else "err:" + t("propose")[:200])
print("refused", "yes" if "approved" in t("route_refused") and (r.get("route_refused", {}).get("error") or "nothing" in t("route_refused")) else "no:" + t("route_refused")[:200])
print("asked", "yes" if "ask" in t("approve_ask") else "no:" + t("approve_ask")[:200])
PY
  cat "$OUT/logs/plan1-checks.txt"
  grep -q "^propose ok" "$OUT/logs/plan1-checks.txt" && ok "plan_propose put the agent's plan into KiCad" || bad "plan_propose: $(sed -n 1p "$OUT/logs/plan1-checks.txt")"
  grep -q "^refused yes" "$OUT/logs/plan1-checks.txt" && ok "plan_route is refused while nothing is approved" || bad "plan_route was not refused: $(sed -n 2p "$OUT/logs/plan1-checks.txt")"
  sleep 2; hub rpc plan.approve "{\"path\":\"$PCB\",\"ids\":[\"c-bare\"],\"approve\":true}" >"$OUT/logs/plan-approve.json"
  grep -q '"ok":true' "$OUT/logs/plan-approve.json" && ok "the person's approval went to KiCad" || bad "plan.approve: $(head -c 200 "$OUT/logs/plan-approve.json")"
  sleep 3; hub rpc turn.start "{\"thread\":\"$tid\",\"text\":\"plan-route\"}" >/dev/null
  for _ in $(seq 240); do [ -s "$P2" ] && break; sleep 0.25; done; cp "$P2" "$OUT/logs/" 2>/dev/null; shot 18-plan-routed
  python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); t=json.dumps(r.get("route",{})); sys.exit(0 if not r.get("route",{}).get("error") and "not approved" not in t and "nothing in the plan" not in t else 1)' "$P2" 2>/dev/null \
    && ok "plan_route of the approved corridor went through ($(python3 -c 'import json,sys; print(json.dumps(json.load(open(sys.argv[1])).get("route"))[:160])' "$P2"))" \
    || bad "plan_route after approval: $(head -c 300 "$P2" 2>/dev/null)"
else bad "the plan turn wrote nothing"; fi
fi
markt end2 1; WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 1
