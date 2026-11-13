# Sourced by journey.sh once setup is at First project and KiCad is installed: the project, Edit, Schematic, the agent's co-edit and comment,
# save, DRC, and the stock KiCad compatibility of what was saved. Uses journey.sh's helpers and variables.
KB=${KDIR/#$HH/$PH}   # the runtime as the box sees it

# --- 4. a real-shaped project: the fork's pic_programmer demo, as if the person had it in Documents ---
as_person sh -c "mkdir -p ~/Documents && cp -r $KB/share/kicad/demos/pic_programmer ~/Documents/" >"$OUT/logs/project-files.txt" 2>&1
PROJ=$PH/Documents/pic_programmer; PCB=$PROJ/pic_programmer.kicad_pcb; HPCB=$HH/Documents/pic_programmer/pic_programmer.kicad_pcb
[ -f "$HPCB" ] && ok "pic_programmer copied into ~/Documents" || { bad "no pic_programmer demo in the runtime ($KB/share/kicad/demos)"; return 1; }
in_box sh -c 'command -v git' >/dev/null || cut_ 3 project "git is not on a stock $(cat "$OUT/os.txt") desktop: a project folder is not a repository, so threads get no worktree and design files are not committed (setup should say so, or offer git)"
ev() { python3 - "$HUBH/events.jsonl" "$@" <<'PY'
import json,sys
f,kind,key,val=sys.argv[1:5]
for l in open(f, errors='replace'):
    try: e=json.loads(l)
    except Exception: continue
    if e.get('$')==kind and str(e.get(key,'')).rstrip('/')==val: print(e['id'])
PY
}
# setup's Create and Start, as their buttons send them (ob-project: project.new {path, orAdd}; ob-thread: thread.create {project})
hub rpc project.new "{\"path\":\"$PROJ\",\"orAdd\":true}" >"$OUT/logs/project-new.json"
pid_=; for _ in $(seq 40); do pid_=$(ev ProjectCreated root "$PROJ" | tail -1); [ -n "$pid_" ] && break; sleep 0.5; done
[ -n "$pid_" ] && { ok "the project was added ($pid_)"; tm project-added "$(now)"; } || { bad "project.new made no project: $(cat "$OUT/logs/project-new.json")"; return 1; }
hub rpc thread.create "{\"project\":\"$pid_\",\"title\":\"Board\"}" >"$OUT/logs/thread-create.json"
tid=; for _ in $(seq 40); do tid=$(ev ThreadCreated project "$pid_" | tail -1); [ -n "$tid" ] && break; sleep 0.5; done
[ -n "$tid" ] && ok "a thread in the project ($tid)" || { bad "no thread: $(cat "$OUT/logs/thread-create.json")"; return 1; }
for _ in 1 2 3; do X key Return; sleep 1.5; done; shot 05-done
[ "$(setting onboard.done)" = 1 ] && { ok "setup finished"; tm setup-done "$(now)"; } || cut_ 3 setup "setup did not finish with Next/Enter from First project (step '$(step)')"

# --- 5. open it: Edit (the real KiCad in the pane), Schematic, back to Board; the agent co-edits meanwhile ---
WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
# reopening Backplane as a person does (no arguments) goes straight to this computer's hub: no chooser, no pairing
: >"$HH/win.log"; launch; sleep 8; shot 05b-reopen
if pos first-use >/dev/null; then bad "reopening shows the data-folder chooser again"; cut_ 1 reopen "reopening Backplane asks for the data folder again"
elif grep -q '"id":"hub:menu"' "$HH/win.log"; then ok "reopening goes straight to this computer's hub"
else bad "reopening did not reach the hub (no main window)"; cut_ 1 reopen "reopening Backplane does not reach its hub: $(tail -2 "$HH/win.log" | cut -c1-200)"; fi
WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
cat >"$HH/journey.json" <<J
[{"wait":3000},{"click":"project:$pid_"},{"wait":2000},{"click":"page-thread:$tid"},{"wait":2500},{"click":"rail:Board"},{"wait":2500},{"mark":"board-view"},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"pcb"}},{"mark":"pcb"},{"kicad":{"report":true,"tag":"pcb"}},{"wait":1500},
 {"click":"rail:Schematic"},{"kicad":{"wait_painted":60000,"kind":"SCH","tag":"sch"}},{"mark":"sch"},{"kicad":{"report":true,"tag":"sch"}},{"wait":1500},
 {"click":"rail:Board"},{"kicad":{"wait_painted":30000,"kind":"PCB","tag":"back"}},{"mark":"back"},{"kicad":{"report":true,"tag":"back"}},
 {"wait":150000},{"kicad":{"report":true,"tag":"end"}},{"mark":"end"}]
J
: >"$HH/win.log"; T1=$(date +%s.%N)
launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/journey.json   # a script with no --url goes to pairing (hub.rs)
markt() { for _ in $(seq ${2:-600}); do grep -q "\"name\":\"$1\"" "$HH/win.log" 2>/dev/null && return 0; sleep 0.25; done; return 1; }
since() { python3 -c "import time;print(round(time.time()-$T1,1))"; }
# a step's own time, by the client's clock: from the click that starts it to the script mark that ends it (both carry epoch_ms), so no
# poll interval and nothing of the steps before it; empty when either is missing
step_s() { python3 - "$HH/win.log" "$1" "$2" <<'PYS'
import json,sys
log,click,mark=sys.argv[1:]; c=m=None
for l in open(log,errors='replace'):
    i=l.find('GPUI_METRIC {')
    if i<0: continue
    try: e=json.loads(l[i+12:])
    except Exception: continue
    if e.get('event')=='mark' and e.get('name')==mark: m=e.get('epoch_ms'); break
    if e.get('event')=='click' and e.get('id')==click: c=e.get('epoch_ms')
print(round((m-c)/1000,1) if c and m else '')
PYS
}
tms() { local v; v=$(step_s "$2" "$3"); [ -n "$v" ] && tm "$1" "$v" || tm "$1" "$4"; }
report() { grep -h '"event":"kicad"' "$HH/win.log" | grep "\"tag\":\"$1\"" | tail -1; }
painted() { report "$1" | python3 -c '
import json,sys
l=sys.stdin.read(); r=json.loads(l[l.index("{"):]) if "{" in l else {}
sh=[f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")]
print("%s %s %s" % (r.get("state"), r.get("container",{}).get("painted",0), (sh[0].get("kind") if sh else "-")))' 2>/dev/null; }
if markt board-view 120; then tm board-viewer "$(since)"; shot 06-board-view; ok "the project's board opens in the viewer"; else bad "the board viewer never came (thread:$tid, rail:Board)"; fi
if markt pcb 600; then E_T=$(since); tms edit-pcb-painted ui:Edit pcb "$E_T"; sleep 1; shot 07-edit-pcb; set -- $(painted pcb)
  if [ "${1:-}" = ready ] && [ "${2:-0}" -gt 50 ]; then ok "Edit: KiCad's PCB editor drawn in the pane (state $1, painted $2, $3)"
  else bad "Edit: PCB not drawn ($(painted pcb))"; cut_ 1 edit "Edit did not draw KiCad's PCB editor: $(report pcb | cut -c1-300)"; fi
else bad "Edit never reported a painted PCB"; cut_ 1 edit "Edit never showed KiCad (no report in 150 s): $(grep -h 'kicad' "$HH/win.log" | tail -2 | cut -c1-300)"; fi
# which KiCad runs: the pid the pane reports, its executable inside the box (nothing from the harness's shell reaches the box: docker exec -e only)
KPID=$(report pcb | python3 -c 'import json,sys
l=sys.stdin.read(); print(json.loads(l[l.index("{"):]).get("pid","") if "{" in l else "")' 2>/dev/null)
if [ -n "$KPID" ]; then KARGS=$(in_box sh -c "tr '\\0' ' ' </proc/$KPID/cmdline" 2>/dev/null | cut -c1-400)
  # the program is the runtime's own loader running bin/kicad.real (the container gives no ptrace for /proc/PID/exe): its command line says which
  KEXE=$(printf '%s' "$KARGS" | tr ' ' '\n' | grep -m1 '/bin/kicad')
  echo "running KiCad: pid $KPID $KEXE; args $KARGS" | tee "$OUT/kicad-running.txt"
  case $KEXE in "$KB"/*) ok "the KiCad in the pane is the runtime setup installed ($KEXE)";; *) bad "the KiCad in the pane is not setup's runtime: $KEXE"; cut_ 2 runtime "a fresh hub runs a KiCad other than the one setup installed: $KEXE";; esac
  HPID=$(cut -d' ' -f1 "$HUBH/hub.lock" 2>/dev/null)
  in_box sh -c "tr '\\0' '\\n' </proc/$HPID/environ" 2>/dev/null | grep '^BACKPLANE_KICAD' >"$OUT/logs/hub-kicad-env.txt"
  [ -s "$OUT/logs/hub-kicad-env.txt" ] && bad "the hub has $(tr '\n' ' ' <"$OUT/logs/hub-kicad-env.txt")" || ok "the hub has no BACKPLANE_KICAD* variable (it picked the runtime itself)"
fi
# budgets in quarter seconds: the first schematic took 82 s once (KiCad's "Load Schematic" twice), 42 s another time
phase "Schematic"; if markt sch 720; then S_T=$(since); tms schematic-painted rail:Schematic sch "$S_T"; sleep 1; shot 08-schematic; set -- $(painted sch)
  # the pane must show the schematic frame, not just be ready (rerun 3: SCH made, never shown, PCB still in the pane); KiCad's own
  # "Load Schematic" progress dialogs are counted, on which runtime
  NLOAD=$(grep -h '"kicad_info"' "$HH/win.log" | grep -o '"dialogs":\[[^]]*\]' | grep -c 'Load Schematic')
  echo "schematic: $* after the click; Load Schematic seen in $NLOAD reports; runtime $(basename "$KDIR")" | tee "$OUT/logs/schematic.txt"
  if [ "${1:-}" = ready ] && [ "${3:-}" = SCH ]; then ok "Schematic: shown ($*)"
  else bad "Schematic clicked, but the pane shows ${3:--} ($*; $(basename "$KDIR"))"; cut_ 1 schematic "Schematic on a fresh install left the pane on ${3:--}: the schematic editor was made but not shown ($(basename "$KDIR"))"; fi
  S_D=$(python3 -c "print(round($S_T - ${E_T:-$S_T}))")
  [ "$S_D" -gt 30 ] && { bad "the first Schematic took $S_D s after the PCB editor (must-fix: under 30 s)"; cut_ 1 schematic "the first Schematic took $S_D s after the PCB editor on a fresh install ($(basename "$KDIR"))"; }
else bad "Schematic never reported in 180 s"; fi
phase "back to the PCB, the agent's turn"; markt back 800 && { sleep 1; shot 09-back-before-agent; }
# the agent's turn (stand-in): kicad_ipc on the live board, save, a comment
TRACKS0=$(grep -c "(segment" "$HPCB")
hub rpc turn.start "{\"thread\":\"$tid\",\"text\":\"add a track beside the first part, save, and comment\"}" >"$OUT/logs/turn-start.json"
CO=$HH/coedit.json; for _ in $(seq 480); do [ -s "$CO" ] && break; sleep 0.25; done
if [ -s "$CO" ]; then
  tm agent-coedit "$(since)"; cp "$CO" "$OUT/logs/"; sleep 2; shot 10-after-agent
  python3 - "$CO" >"$OUT/logs/coedit-checks.txt" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]))
def body(k):
    b=r.get(k,{}).get('body'); return b if isinstance(b,dict) else {}
n=lambda k:len((body(k).get('message') or {}).get('items') or [])
e=lambda k:'no' if r.get(k,{}).get('error') or k not in r else 'yes'
op='yes' if e('open')=='yes' or e('open_docs')=='yes' else 'no'
print("ok", op, e('create'), e('save'), e('comment'))
print("tracks", n('before'), n('after'))
for k in ('status','lock','open_docs','create','save','comment','comments'):
    if r.get(k,{}).get('error') or k not in r: print("ERR", k, json.dumps(r.get(k))[:400])
PY
  cat "$OUT/logs/coedit-checks.txt"
  read -r _ o c sv cm <"$OUT/logs/coedit-checks.txt"; read -r _ b a < <(sed -n 2p "$OUT/logs/coedit-checks.txt")
  [ "$o $c" = "yes yes" ] && [ "$a" = $((b+1)) ] && ok "the agent's kicad_ipc commit landed in the live board ($b -> $a tracks)" || bad "agent co-edit: $(grep ERR "$OUT/logs/coedit-checks.txt" | head -2 | tr '\n' ' ') tracks $b -> $a"
  PANE=$(report back | python3 -c 'import json,sys
l=sys.stdin.read(); c=json.loads(l[l.index("{"):]).get("container",{}) if "{" in l else {}
print("%dx%d+%d+%d" % (c.get("w",0), c.get("h",0), c.get("root_x",0), c.get("root_y",0)) if c.get("w") else "")' 2>/dev/null)
  python3 "$HERE/imgdiff.py" "$OUT/shots/09-back-before-agent.png" "$OUT/shots/10-after-agent.png" 40 "$PANE" >"$OUT/logs/agent-diff.txt" 2>&1 && ok "the agent's track showed in the pane ($(cat "$OUT/logs/agent-diff.txt") in ${PANE:-the whole screen})" || bad "the pane did not change after the agent's commit ($(cat "$OUT/logs/agent-diff.txt"))"
  [ "$cm" = yes ] && ok "the agent's comment posted" || bad "review_comment failed"
  [ "$sv" = yes ] && ok "SaveDocument answered" || bad "SaveDocument failed"
  TRACKS1=$(grep -c "(segment" "$HPCB")
  [ "$TRACKS1" -gt "$TRACKS0" ] && ok "the saved board on disk has the agent's track ($TRACKS0 -> $TRACKS1 segments)" || bad "the board on disk did not get the track ($TRACKS0 -> $TRACKS1)"
else bad "the agent's turn wrote nothing in 120 s"; cut_ 1 agent "an agent's turn on the open board did nothing: $(tail -3 "$HUBH/hub.log" 2>/dev/null | tr '\n' ' ' | cut -c1-300)"; fi
markt end 400; WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null

# --- 5b. the management panels, cross-probe, comment window, Gerbers, plan tools (journey_more.sh) ---
. "$HERE/journey_more.sh"

# --- 6. DRC with the runtime's kicad-cli on the saved board ---
t=$(date +%s.%N)
as_person "$KB/bin/kicad-cli" pcb drc --format json --severity-all -o $PH/drc.json "$PCB" >"$OUT/logs/drc.log" 2>&1
if [ -s "$HH/drc.json" ]; then cp "$HH/drc.json" "$OUT/logs/"; ok "DRC by the runtime's kicad-cli: $(python3 -c "import json;j=json.load(open('$HH/drc.json'));print(len(j.get('violations',[])),'violations,',len(j.get('unconnected_items',[])),'unconnected')")"
else bad "DRC did not run: $(tail -2 "$OUT/logs/drc.log")"; fi
tm drc "$(python3 -c "import time;print(round(time.time()-$t,1))")"

# --- 7. stock KiCad: the current release from its official packages, on the fork's exports ---
t=$(date +%s.%N)
# Launchpad's API flakes (rerun 7: a broken HTTP chunk in add-apt-repository): three tries
ppa_add() { local i; for i in 1 2 3; do in_box add-apt-repository -y ppa:kicad/kicad-10.0-releases >>"$OUT/logs/box-pkg.log" 2>&1 && return 0; sleep 10; done; return 1; }
if [ $PM = apt ]; then pkg_install software-properties-common gpg-agent && ppa_add \
  && in_box apt-get update -qq >>"$OUT/logs/box-pkg.log" 2>&1 && pkg_install --no-install-recommends kicad kicad-symbols kicad-footprints kicad-templates; fi   # `apt install kicad` brings these as recommends (and the 3D models, left out: GBs)
# a version, not docker's "executable file not found" (on stdout: rerun 7 printed it as a PASS)
SV=$(in_box kicad-cli --version 2>/dev/null | grep -E '^[0-9]+\.[0-9]' | head -1); tm stock-kicad-install "$(python3 -c "import time;print(round(time.time()-$t,1))")"
t=$(date +%s.%N)
if [ -n "$SV" ]; then
  echo "$SV" >"$OUT/stock-version.txt"; ok "stock KiCad $SV from the official packages"
  as_person sh -c "rm -rf ~/compat-src && mkdir ~/compat-src && cp -r ~/Documents/pic_programmer ~/compat-src/ && for d in ecc83 complex_hierarchy video; do cp -r $KB/share/kicad/demos/\$d ~/compat-src/ 2>/dev/null; done; true"
  cp "$HERE/stock_compat.py" "$HERE/stock_engine_known.json" "$HH/"
  as_person python3 -I $PH/stock_compat.py --fork-cli "$KB/bin/kicad-cli" --stock-cli /usr/bin/kicad-cli --as-is --out $PH/compat-saved $PH/compat-src/pic_programmer >"$OUT/logs/compat-saved.log" 2>&1
  as_person sh -c "python3 -I ~/stock_compat.py --fork-cli '$KB/bin/kicad-cli' --stock-cli /usr/bin/kicad-cli --out ~/compat-demos \$(ls -d ~/compat-src/* | grep -v pic_programmer)" >"$OUT/logs/compat-demos.log" 2>&1
  for f in compat-saved compat-demos; do
    grep -E "^(PASS|FAIL) [^:]+: [0-9]+/[0-9]+ checks|^FAIL|^NOTE" "$OUT/logs/$f.log" | sed 's/^/  /'
    cp "$HH/$f/summary.json" "$OUT/logs/$f-summary.json" 2>/dev/null
    grep -q "^FAIL" "$OUT/logs/$f.log" && bad "stock compatibility ($f): $(grep -c '^FAIL' "$OUT/logs/$f.log") checks failed" || ok "stock compatibility ($f): all pass"
  done
else bad "stock KiCad did not install from the official packages ($(tail -2 "$OUT/logs/box-pkg.log" | tr '\n' ' '))"; fi
tm stock-compat "$(python3 -c "import time;print(round(time.time()-$t,1))")"

# --- 8. a DRC correctness probe: a short inside a zone fill (test/bare/drc_probe.py; a miss FAILs) ---
cp "$HERE/drc_probe.py" "$HH/"
SC=; [ -n "${SV:-}" ] && SC="--stock-cli /usr/bin/kicad-cli"
as_person python3 -I $PH/drc_probe.py --fork-cli "$KB/bin/kicad-cli" $SC --demo "$KB/share/kicad/demos/complex_hierarchy" --out $PH/drc-probe >"$OUT/logs/drc-probe.log" 2>&1
cp "$HH/drc-probe/result.json" "$OUT/logs/drc-probe.json" 2>/dev/null
while IFS= read -r l; do case $l in
  PASS*) ok "${l#PASS }";; FAIL*) bad "${l#FAIL }";;
  NOTE*) echo "$l";; esac; done <"$OUT/logs/drc-probe.log"
grep -qE '^(PASS|FAIL)' "$OUT/logs/drc-probe.log" || bad "the DRC probe did not run: $(tail -2 "$OUT/logs/drc-probe.log")"
