#!/bin/sh
# A Rust hub of our own for the phone checks (stand-in agents, loopback only, own scope), seeded with a project and two threads.
# usage: scripts/mobile/data-hub.sh start | stop | link     (PORT default 3841; HUBBIN default the gpui checkout's release build)
set -eu
BUN=$(command -v bun)   # before PATH is cut down below
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
H=${H:-$ROOT/build/mobile/hub}; PORT=${PORT:-3841}; HOST=${HOST:-127.0.0.1}; HUBBIN=${HUBBIN:-$ROOT/build/mobile/target/fast/backplane-hub}
case "${1:-}" in
start)
  rm -rf $H/home $H/proj; mkdir -p $H/home $H/proj $H/bin
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $H/bin/claude; chmod +x $H/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $H/bin/$n; chmod +x $H/bin/$n; done
  echo "# demo" > $H/proj/README.md
  # a KiCad project (the hub's own plot fixtures), a model, a picture, a PDF and a note: what the viewer tabs show
  F=$ROOT/test/files/fixtures; mkdir -p $H/proj/mech $H/proj/docs
  cp $F/plot/ecc83-pp.kicad_pcb $H/proj/ecc83.kicad_pcb; cp $F/plot/ampli_ht.kicad_sch $H/proj/ecc83.kicad_sch; echo '{}' > $H/proj/ecc83.kicad_pro
  cp $F/mesh/bracket.step $H/proj/mech/bracket.step; cp $ROOT/test/mobile/pic.png $H/proj/docs/picture.png; cp $ROOT/test/mobile/note.pdf $H/proj/docs/note.pdf
  # a git repository with one change, for the Diff tab
  (cd $H/proj && git init -q && git add -A && git -c user.email=t@t -c user.name=t commit -qm "start" && printf 'The tube socket moves 2 mm left.\n' >> README.md && echo '{}' > $H/proj/.gitignore.tmp && rm $H/proj/.gitignore.tmp)
  mkdir -p $H/home/shots; [ "${LIVE:-}" = 1 ] || cp $ROOT/test/mobile/browser-frame.jpg $H/home/shots/t1.jpg   # a browser page's newest frame for the thread t1
  printf '# Notes\n\nThe tube socket moves 2 mm left.\n' > $H/proj/docs/notes.md
  # LINKS=1: a file the link check opens (src/main.rs, 24 lines) and a thread "Links" whose messages hold every kind of link (test/mobile/links-fixture.md)
  mkdir -p $H/proj/src; i=1; while [ $i -le 24 ]; do echo "// line $i of src/main.rs" >> $H/proj/src/main.rs; i=$((i+1)); done
  ROOT=$ROOT python3 - "$H" <<'PY'
import json,sys,time
H=sys.argv[1]; N=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"demo","root":H+"/proj","at":N},
    {"$":"ThreadCreated","id":"t1","project":"p1","title":"Fix the ecc83 board","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},
    {"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"Move the tube socket 2mm left","at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a0","tone":"step","kind":"step","summary":'{"p":"","w":"start"}',"at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a1","tone":"tool","kind":"Read","summary":"ecc83.kicad_pcb","at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a2","tone":"tool","kind":"Bash","summary":"kicad-cli pcb drc ecc83.kicad_pcb --output drc.rpt","at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a3","tone":"tool","kind":"mcp__backplane__viewer_show","summary":"ecc83.kicad_pcb","at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a4","tone":"tool","kind":"TodoWrite","summary":"TodoWrite","at":N},
    {"$":"ActivityLogged","thread":"t1","act":"a5","tone":"step","kind":"step","summary":'{"p":"abc","w":"end","f":["M\\tecc83.kicad_pcb"]}',"at":N},
    {"$":"MessagePosted","thread":"t1","msg":"m2","role":"assistant","text":"Done. I moved **J1** two millimetres left and re-ran DRC.\n\n- `J1` at 42.0, 31.5\n- 0 errors, 2 warnings\n\n```\nkicad-cli pcb drc ecc83.kicad_pcb\n```","at":N+1},
    {"$":"ThreadCreated","id":"tb1","project":"","title":"ada","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},
    {"$":"BotSet","id":"b1","name":"ada","thread":"tb1","look":3,"persona":"","at":N},
    {"$":"RoomSet","id":"r1","name":"lab","members":"ada","at":N},
    {"$":"ThreadCreated","id":"t2","project":"p1","title":"BOM review","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N}]
import os
if os.environ.get("LINKS") == "1":
    fx=open(os.path.join(os.environ["ROOT"],"test/mobile/links-fixture.md")).read().rstrip("\n")
    ev+=[{"$":"ThreadCreated","id":"t3","project":"p1","title":"Links","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N+2},
         {"$":"MessagePosted","thread":"t3","msg":"m1","role":"user","text":"Where is it? backplane://thread/t2 and src/main.rs:5, mail me@example.com","at":N+2},
         {"$":"MessagePosted","thread":"t3","msg":"m2","role":"assistant","text":fx,"at":N+3}]
open(H+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
  # STEP models: the console FreeCAD only (the hub tries FreeCAD's GUI first, which hangs headless on this machine: no `freecad` on the hub's PATH, no GUI to find)
  FC=$(command -v freecadcmd || true); mkdir -p $H/fc; [ -n "$FC" ] && printf '#!/bin/sh\nexec %s "$@"\n' "$FC" > $H/fc/freecadcmd && chmod +x $H/fc/freecadcmd && export BACKPLANE_FREECAD=$H/fc/freecadcmd && export PATH=/usr/bin:/bin
  # LIVE=1: the real browser helper (tools/browser on Bun, Playwright driving the machine's Chrome) instead of a planted frame
  # otherwise a stand-in helper: it answers every request and writes each one to $H/browser-ops.jsonl, so the protocol check sees what the person's input became
  if [ "${LIVE:-}" != 1 ]; then : > $H/browser-ops.jsonl
    cat > $H/bin/backplane-browser <<EOF
#!/bin/sh
while IFS= read -r l; do
  printf '%s\n' "\$l" >> $H/browser-ops.jsonl
  id=\$(printf '%s' "\$l" | sed 's/.*"id":\([0-9]*\).*/\1/')
  case "\$l" in *'"op":"status"'*) printf '{"id":%s,"ok":true,"result":{"width":1280,"height":800}}\n' "\$id" ;; *) printf '{"id":%s,"ok":true,"result":{}}\n' "\$id" ;; esac
done
EOF
    chmod +x $H/bin/backplane-browser; export BACKPLANE_BROWSER=$H/bin/backplane-browser; fi
  [ "${LIVE:-}" = 1 ] && { printf '#!/bin/sh\nexport BACKPLANE_CHROME=${CHROME:-/usr/bin/google-chrome}\nexec %s %s/tools/browser/src/main.ts "$@"\n' "$BUN" "$ROOT" > $H/bin/backplane-browser; chmod +x $H/bin/backplane-browser; export BACKPLANE_BROWSER=$H/bin/backplane-browser; }
  export BACKPLANE_AGENTS=$H/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=${STANDIN:-stream} BP_STREAM_N=8 BP_STREAM_MS=200
  cd $H/proj
  # a scope, not a service: it keeps this environment (a service of the user manager gets none of it, and the hub then finds the real agent CLIs), and $! is the hub
  nohup systemd-run --user --scope --unit=bp-data-hub-$PORT -q --collect nice -n 10 $HUBBIN --home $H/home --port $PORT --host $HOST --no-tailscale --foreground > $H/hub.log 2>&1 &
  echo $! > $H/pid; sleep 4
  # stand-ins only: a hub without them is stopped at once
  tr '\0' '\n' < /proc/$(cat $H/pid)/environ 2>/dev/null | grep -qx "BACKPLANE_AGENTS=$H/bin" || { echo "data-hub: the hub has no BACKPLANE_AGENTS=$H/bin; stopped" >&2; kill "$(cat $H/pid)" 2>/dev/null; rm -f $H/pid; exit 1; }
  curl -s -m3 http://$HOST:$PORT/hello | head -c 160; echo ;;
link) echo "http://${LINKHOST:-10.0.2.2}:$PORT/#token=$(cat $H/home/token)" ;;
stop) kill "$(cat $H/pid)" 2>/dev/null || true; rm -f $H/pid; echo stopped ;;
esac
