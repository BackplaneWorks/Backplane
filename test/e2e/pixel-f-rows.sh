#!/bin/bash
# The phone halves of F06, F07, F17 and F21 on the Pixel 9 (dev.backplane.mobile.gpui, landscape 2424x1080), through real touches (adb input) against a private hub on a temp home:
#   flock -o ~/.cache/pixel-52220DLAQ001FC.lock test/e2e/pixel-f-rows.sh HUB_BIN OUT_DIR
#   (-o: the lock's fd is not inherited, so a hub this script starts never holds the phone's lock; its hubs are found by their home, whatever the binary is called)
# Taps are in the 2000-wide frame screenshots are shown in (x1.21 to the screen). A check passes from the hub's events.jsonl or the stand-in's log, or from changed pixels on the viewer; never from the driver's say-so.
# Never touches the iPhone; only the gpui app package (dev.backplane.mobile.gpui, never the person's dev.backplane.mobile). The phone reaches the private hub over the LAN
# The hub listens on loopback; the phone reaches it through `adb reverse` (removed when the rows end). The app refuses an invitation that names only 127.0.0.1 ("this phone cannot reach it"),
# so the invitation also names this machine's LAN address (PIXEL_HOST, default: the address on the route to the internet), which the desktop's firewall refuses: the app pairs and
# connects through the loopback route, i.e. the tunnel.
set -u; HUB=$(realpath "$1"); O=$2; PORT=${PORT:-3893}; S=52220DLAQ001FC; PKG=dev.backplane.mobile.gpui; HERE=$(cd "$(dirname "$0")" && pwd)
A() { adb -s $S "$@"; }; shot() { A exec-out screencap -p > "$O/shots/$1.png"; }
T() { A shell input tap $(python3 -c "print(int($1*1.21), int($2*1.21))"); }
res=(); ok() { echo "$1 $2 $3"; res+=("{\"id\":\"$2\",\"ok\":$( [ "$1" = ok ] && echo true || echo false),\"ev\":\"$3\"}"); }
export PIXEL_HOST=${PIXEL_HOST:-$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src") print $(i+1)}')}
[ -n "$PIXEL_HOST" ] || { echo "no LAN address for the hub (set PIXEL_HOST)"; exit 2; }
rm -rf "$O"; mkdir -p "$O/shots"; (bun "$HERE/pixel-hub.mjs" "$HUB" "$O/hub" $PORT > "$O/hub.out" 2>&1 &); sleep 8
EV=$O/hub/home/events.jsonl; F=$O/hub/fake.log; HP=$(python3 -c "import json;print(json.load(open('$O/hub/state.json'))['pid'])")
cleanup() { A reverse --remove tcp:$PORT >/dev/null 2>&1; kill $HP $(pgrep -f -- "--home $O/hub/home") 2>/dev/null; pkill -f "pixel-hub.mjs $HUB $O/hub" 2>/dev/null; echo "{\"rows\":[$(IFS=,; echo "${res[*]}")]}" > "$O/rows.json"; }; trap cleanup EXIT
A reverse tcp:$PORT tcp:$PORT >/dev/null
A shell pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1   # a cleared app asks for it at its first screen and the prompt would cover the rows (this package only)
A shell am force-stop $PKG
INV=$(python3 -c "import json,urllib.parse;print(urllib.parse.quote(json.load(open('$O/hub/state.json'))['invite'],safe=''))")
ime_off() { local i; for i in 1 2 3; do A shell dumpsys input_method 2>/dev/null | grep -q 'mInputShown=true' || return 0; A shell input keyevent KEYCODE_BACK; sleep 1; done; }   # a keyboard left up by an earlier run covers the buttons
A shell am start -W -a android.intent.action.VIEW -d "bpgpui://unpair" $PKG >/dev/null; sleep 3; ime_off      # start from the pairing screen, whatever the app held
A shell am start -W -a android.intent.action.VIEW -d "bpgpui://pair?url=$INV" $PKG >/dev/null; sleep 6; ime_off; shot pair-prompt; T 301 560; sleep 8; shot sidebar
T 170 603; sleep 3                       # Thread 1 (approval-required)
# the thread's composer is on screen (its Attach and Send controls): typing goes nowhere else (a run that typed into the pairing field left "zqapprove now" there)
on_thread() { A shell uiautomator dump /sdcard/rows-ui.xml >/dev/null 2>&1; A shell cat /sdcard/rows-ui.xml 2>/dev/null | grep -q 'content-desc="Attach"' && A shell cat /sdcard/rows-ui.xml | grep -q 'Send'; }
say() { on_thread || { echo "say: the thread's composer is not on screen; '$1' not typed"; shot not-on-thread; return 1; }; T 1000 670; sleep 0.7; A shell input text "$1"; sleep 0.7; A shell input keyevent KEYCODE_BACK; sleep 1; T 1907 779; }
on_thread || { shot not-paired; ok FAIL setup "the app is not on Thread 1 of this hub after pairing (shots/not-paired.png): nothing was typed, no row ran"; exit 1; }
wait_for() { for i in $(seq 1 ${2:-25}); do eval "$1" && return 0; sleep 1; done; return 1; }
say 'zqapprove%snow'; wait_for "[ \$(grep -c AskOpened $EV) -ge 1 ]"; sleep 2; shot approval; T 500 303; wait_for "grep -q 'PERM Bash.*allow' $F"
grep -q 'PERM Bash.*allow' $F && ok ok F06-phone-allow "the approval card on the phone: Allow tapped, the agent got allow" || ok FAIL F06-phone-allow "no allow reached the agent"
sleep 4; say 'zqapprove%sagain'; wait_for "[ \$(grep -c AskOpened $EV) -ge 2 ]"; sleep 2; T 500 503; wait_for "grep -q 'PERM Bash.*deny' $F"
grep -q 'PERM Bash.*deny' $F && ok ok F06-phone-decline "Decline tapped, the agent got deny" || ok FAIL F06-phone-decline "no deny reached the agent"
sleep 3; n=$(grep -c '"state":"running"' $EV); say 'a%slong%sturn'; wait_for "[ \$(grep -c '\"state\":\"running\"' $EV) -gt $n ]"; sleep 2; shot running; T 1907 779
wait_for "grep -q '\"state\":\"interrupted\"' $EV" && ok ok F07-phone-stop "Stop tapped on a running turn: TurnChanged interrupted in the hub log" || ok FAIL F07-phone-stop "not interrupted"
sleep 3; T 1835 104; sleep 4; D=$(python3 -c "print(' '.join(str(int(v*1.21)) for v in (700,500,1300,560)))"); views=""
PS=$HERE/lib/phone_shot.py; views=""
ready() { python3 -c "
import sys,json;d=json.loads(sys.argv[2]);k=sys.argv[1]
ok=d['bright']>6000 and d['still'] and ((d['green']>0.25) if k=='3D' else (d['green']<0.15) if k=='Mech' else True)
sys.exit(0 if ok else 1)" "$1" "$2"; }
for v in "Board 70 211" "Schematic 250 211" "3D 403 211" "Mech 660 211"; do set -- $v; kind=$1; T $2 $3
  # ready = something is drawn, nothing moves without a touch (a spinner or a redrawing loading card would), and it is the right thing: the board's 3D page is the green board model,
  # the Mech page is the grey part (not the board's model, not a "Loading" card); a card that sits there still fails the swipe test below
  st='{"bright":0,"green":0,"blue":0,"still":false}'
  for k in $(seq 1 14); do sleep 4; A shell input swipe 1210 680 1212 682 60; sleep 1; shot v-$kind; sleep 2; shot v-$kind-b; st=$(python3 -W ignore $PS "$O/shots/v-$kind.png" "$O/shots/v-$kind-b.png"); ready $kind "$st" && break; done
  A shell input swipe $D 500; sleep 2; shot v-$kind-moved
  mv=$(python3 -W ignore -c "
from PIL import Image,ImageChops
h=lambda n:Image.open('$O/shots/%s.png'%n).convert('RGB').crop((0,330,2424,1000))
print('moved' if ImageChops.difference(h('v-$kind'),h('v-$kind-moved')).getbbox() else 'unmoved')")
  if ready $kind "$st"; then c="ready+$mv"; else c="notready$st+$mv"; fi; views="$views $kind:$c"; done
# the Mech page must not be the board's model: its picture differs from the 3D page's
mm=$(python3 -W ignore -c "
from PIL import Image,ImageChops
h=lambda n:Image.open('$O/shots/%s.png'%n).convert('RGB').crop((0,330,2424,1000))
print('differs' if ImageChops.difference(h('v-3D'),h('v-Mech')).getbbox() else 'same-as-3D')"); views="$views Mech-vs-3D:$mm"
case "$views" in *notready*|*unmoved*|*same-as-3D*) ok FAIL F17-phone "viewer pages on the phone:$views";; *) ok ok F17-phone "viewer pages on the phone (each settled, nothing animating, the right content, and a swipe changed it; Mech is the part and not the board):$views";; esac
T 60 104; sleep 3; say 'zqask'; wait_for "grep -q '\"kind\":\"input\"' $EV"; sleep 3; shot question; T 135 503; wait_for "grep -q 'PERM AskUserQuestion.*Barrel' $F"
grep -q 'PERM AskUserQuestion.*Barrel' $F && ok ok F06-phone-question "after Stop, the next turn's question is drawn on the phone (shots/question.png: Which port? with USB-C and Barrel) and Barrel tapped reaches the agent" || ok FAIL F06-phone-question "no answer reached the agent after the question"
sleep 2; kill $HP; sleep 6; shot offline; pkill -f -- "--home $O/hub/home"; sleep 1
(cd $O/hub/proj && BACKPLANE_AGENTS=$O/hub/bin PATH=$O/hub/bin:$PATH BACKPLANE_NO_UPDATE=1 SHELL=/bin/sh FAKE_LOG=$F HOME=$O/hub/home nohup "$HUB" --home $O/hub/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $O/hub2.log 2>&1 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- &); sleep 20
n=$(grep -c "after the reconnect" $EV); say 'zqshort%safter%sthe%sreconnect'; wait_for "[ \$(grep -c 'after the reconnect' $EV) -gt $n ]" 30 && ok ok F21-phone "the hub was killed (the phone showed its offline banner, shots/offline.png) and started again on the same home: a message sent from the phone afterwards reached the new hub process" || ok FAIL F21-phone "no message reached the restarted hub"
shot final
# leave the app as the next lane finds it: unpaired, at the pairing screen
A shell am start -W -a android.intent.action.VIEW -d "bpgpui://unpair" $PKG >/dev/null; sleep 4; shot unpaired
A shell pm clear $PKG >/dev/null; A shell pm grant $PKG android.permission.POST_NOTIFICATIONS 2>/dev/null   # and nothing typed left in its fields
