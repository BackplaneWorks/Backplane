#!/bin/bash
# windowed_e2e.sh CLIENT_BIN HUB_BIN [OUT]: the Browser tab's windowed Chromium on a private Xvfb with a window manager (i3): Chromium's own native
# window sits inside the client's window over the Browser pane, follows the layout and the window manager's resizing, hides under a gpui overlay (the Improvements panel) and while
# another view is shown, shows only the active tab, takes the keyboard only on the person's click, and hands the client's own shortcuts back.
# Reads the client's `cef` reports (the child's geometry and map state, read back from the X server) and the X window tree; screenshots per step
# in OUT/<step>.png. -> PASS/FAIL lines, exit 0 when all pass.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); C=$1; H=$2; OUT=${3:-/tmp/cef-windowed}; B=$(dirname "$(readlink -f "$C")")
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/fh" "$OUT/proj" "$OUT/bin"
for n in claude codex grok muse opencode; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
XV=; for d in "${XVFB_ROOT:-}" "$HOME/.cache/xvfb-root" "$HOME/.local/xvfb-root"; do [ -n "$d" ] && [ -x "$d/usr/bin/Xvfb" ] && { XV=$d; break; }; done
XVB=$XV/usr/bin/Xvfb; XLD=$XV/usr/lib/x86_64-linux-gnu; [ -x "$XVB" ] || { XVB=$(command -v Xvfb); XLD=; }
D=$((500 + $$ % 200)); while [ -e /tmp/.X11-unix/X$D ] || [ -e /tmp/.X$D-lock ]; do D=$((D + 1)); done
R=$(mktemp -d /tmp/bpw.XXXX); chmod 700 "$R"; PIDS=; trap 'kill $PIDS 2>/dev/null; rm -rf "$R"' EXIT
LD_LIBRARY_PATH=$XLD "$XVB" :$D -screen 0 1600x1000x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$OUT/xvfb.log" 2>&1 & PIDS="$PIDS $!"; sleep 1.5
export DISPLAY=:$D XDG_RUNTIME_DIR=$R
i3 -c /dev/null > "$OUT/i3.log" 2>&1 & PIDS="$PIDS $!"; sleep 1
P1='data:text/html,<body style="margin:0;background:%23e8f0ff"><h1 style="color:%23c00">first tab</h1><input id=t autofocus></body>'
P2='data:text/html,<body style="margin:0;background:%23e0ffe0"><h1>second tab</h1></body>'
python3 - "$OUT" "$P1" "$P2" <<'PY'
import json,sys,time
o,p1,p2=sys.argv[1:4]; n=int(time.time())
ev=[{'$':'ProjectCreated','id':'p1','title':'web','root':o+'/proj','at':n},{'$':'ThreadCreated','id':'t1','project':'p1','title':'Windowed','env':'local','branch':'','worktree':'','provider':'claude','model':'','at':n}]
open(o+'/home/events.jsonl','w').write(''.join(json.dumps(e)+'\n' for e in ev))
step=lambda name:[{'mark':name},{'cef':{'report':True}},{'wait':2500}]
s=[{'select':0},{'wait':2000},{'click':'rail:Browser'},{'wait':5000},{'cef':{'goto':p1}},{'wait':3000}]+step('shown')
s+=[{'keys':'ctrl-b'},{'wait':2000}]+step('sidebar_folded')+[{'keys':'ctrl-b'},{'wait':2000}]
s+=[{"click":"ui:Improvements"},{"wait":1500}]+step("overlay")+[{"keys":"ctrl-alt-i"},{"wait":1500}]+step("overlay_closed")
s+=[{'click':'rail:Files'},{'wait':2000}]+step('other_view')+[{'click':'rail:Browser'},{'wait':2000}]+step('back')
s+=[{'browser':{'op':'new_tab','url':p2}},{'wait':3000}]+step('two_tabs')+[{'cef':{'tab':0}},{'wait':1500}]+step('first_tab_again')
s+=[{'mark':'pre_resize'},{'wait':3000}]+step('resized')
s+=[{'wait':4000}]+step('keyboard')+[{'wait':8000}]
json.dump(s,open(o+'/script.json','w'))
PY
HP=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
HOME=$OUT/fh BACKPLANE_AGENTS=$OUT/bin BACKPLANE_NO_UPDATE=1 "$H" --home "$OUT/home" --port $HP --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.log" 2>&1 & PIDS="$PIDS $!"
for i in $(seq 50); do curl -s -o /dev/null "http://127.0.0.1:$HP/hello" && break; sleep 0.2; done
HOME=$OUT/fh BACKPLANE_GPUI_ASSETS=${ASSETS:-$B/assets} BACKPLANE_CEF_PROFILE=$OUT/prof "$C" --url "http://127.0.0.1:$HP" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 90 > "$OUT/client.log" 2>&1 & CP=$!; PIDS="$PIDS $CP"
# at each mark: the X tree and a screenshot; at `keyboard`: the person clicks into the page, types, and presses the client's Ctrl+J there
seen=
while kill -0 $CP 2>/dev/null; do
  for m in $(grep -o '"name":"[a-z_]*","event":"mark"' "$OUT/client.log" 2>/dev/null | cut -d'"' -f4); do
    case " $seen " in *" $m "*) continue;; esac; seen="$seen $m"; sleep 0.4
    xwininfo -root -tree > "$OUT/$m.tree" 2>&1; import -window root "$OUT/$m.png" 2>/dev/null
    # the window manager makes the window smaller and moves it (a floating window, 1200x760 at 120,90)
    if [ "$m" = pre_resize ]; then i3-msg 'floating enable, resize set 1200 760, move position 120 90' > "$OUT/i3msg.txt" 2>&1; fi
    if [ "$m" = keyboard ]; then
      read X Y W HH <<< "$(grep '"cef"' "$OUT/client.log" | tail -1 | python3 -c 'import json,sys;l=sys.stdin.read();r=json.loads(l[l.index("{"):]);n=r["native"];print(n["x"],n["y"],n["w"],n["h"])')"
      # the window's place on the screen: the last +X+Y of its line in the tree (the first is within the window manager's frame)
      WX=$(xwininfo -root -tree | grep 'Backplane GPUI' | head -1 | grep -o '+[0-9-]*+[0-9-]*' | tail -1 | tr '+' ' '); read GX GY <<< "$WX"
      python3 "$HERE/xinput.py" click $((GX + X + 60)) $((GY + Y + 120)); sleep 0.6
      python3 "$HERE/xinput.py" type typed; sleep 0.4
      python3 "$HERE/xinput.py" chord ctrl j; sleep 1.2
      import -window root "$OUT/keyboard_after.png" 2>/dev/null
    fi
  done; sleep 0.2
done
python3 - "$OUT" <<'PY'
import json,sys,re
o=sys.argv[1]; L=open(o+'/client.log',errors='ignore').read().splitlines()
marks=[]; reps={}
for l in L:
    if '"event":"mark"' in l: marks.append(json.loads(l[l.index('{'):])['name'])
    elif '"event":"cef"' in l and marks:
        r=json.loads(l[l.index('{'):]); reps.setdefault(marks[-1], r)
ok=fail=0
def check(what, good, ev=''):
    global ok, fail
    print(('PASS ' if good else 'FAIL ')+what+('' if good else ' ('+str(ev)[:300]+')')); ok+=bool(good); fail+=not good
mode=[l for l in L if 'cef_mode' in l]; check('the client chose windowed Chromium ('+(mode[0][9:] if mode else 'no cef_mode line')+')', mode and '"windowed"' in mode[0], mode)
def nat(m): r=reps.get(m,{}); return r, (r.get('native') or {})
r,n=nat('shown')
tree=open(o+'/shown.tree',errors='ignore').read()
gp=re.search(r'"Backplane GPUI[^\n]*\n\s+\d+ child(?:ren)?:\n\s+(0x[0-9a-f]+)', tree)
check('Chromium\'s window is a child of the client\'s window (X tree)', bool(gp), tree[-600:])
sc=1.0
check('it is shown over the Browser pane (x %s y %s, pane at %s,%s)'%(n.get('x'),n.get('y'),r.get('view_x'),r.get('view_y')), n.get('shown') and abs(n.get('x',-9)-r.get('view_x',0)*sc)<=1 and abs(n.get('y',-9)-r.get('view_y',0)*sc)<=1 and n.get('w',0)>200, n)
check('the page painted in it (non-white samples %s)'%n.get('painted'), (n.get('painted') or 0)>5, n)
r,n=nat('overlay'); check('a gpui overlay over the pane hides it (covered %s, shown %s)'%(r.get('covered'),n.get('shown')), r.get('covered') and not n.get('shown'), r)
r,n=nat('overlay_closed'); check('closing the overlay shows it again', not r.get('covered') and n.get('shown'), r)
r,n=nat('other_view'); check('another view in the pane hides it (shown %s)'%n.get('shown'), n and not n.get('shown'), r)
r,n=nat('back'); check('back on the Browser tab it is shown again', n.get('shown'), r)
r0,n0=nat('shown'); r,n=nat('sidebar_folded')
check('a layout change moves it with the pane (x %s -> %s, pane %s)'%(n0.get('x'),n.get('x'),r.get('view_x')), n.get('shown') and n.get('x')!=n0.get('x') and abs(n.get('x',-9)-r.get('view_x',0))<=1 and n.get('w',0)>n0.get('w',0), r)
r0,n0=nat('shown'); r,n=nat('resized')
check('the window manager resizing and moving the window: it follows the pane (w %s -> %s, x %s, pane %s)'%(n0.get('w'),n.get('w'),n.get('x'),r.get('view_x')), n.get('shown') and n.get('w',0)<n0.get('w',0) and abs(n.get('x',-9)-r.get('view_x',0))<=1 and abs(n.get('y',-9)-r.get('view_y',0))<=1, (n, open(o+'/i3msg.txt').read()[:200]))
r,_=nat('two_tabs'); ns=r.get('natives',[])
check('with two tabs only the active one is shown (%s)'%[x and x.get('shown') for x in ns], len(ns)==2 and ns[1] and ns[1].get('shown') and ns[0] and not ns[0].get('shown'), ns)
r,_=nat('first_tab_again'); ns=r.get('natives',[])
check('switching tabs swaps them (%s)'%[x and x.get('shown') for x in ns], len(ns)==2 and ns[0] and ns[0].get('shown') and ns[1] and not ns[1].get('shown'), ns)
r,_=nat('shown'); check('opening the Browser tab left the keyboard with the client (not taken: %s)'%r.get('taken'), r.get('taken') is False, r)
keys=[json.loads(l[l.index('{'):]) for l in L if 'cef_key' in l]
check('the person\'s Ctrl+J in the page went to the client first and the client took it (%s)'%keys, any(k.get('cef_key')=='ctrl-j' and k.get('client_took') for k in keys), keys)
print('passed=%d failed=%d; artifacts %s'%(ok,fail,o)); sys.exit(1 if fail else 0)
PY
