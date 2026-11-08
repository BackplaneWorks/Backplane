#!/bin/bash
# The phone halves of F06, F07, F17 and F21 on the Pixel 9 (dev.backplane.mobile.gpui, landscape 2424x1080), through real touches (adb input) against a private hub on a temp home:
#   flock -o ~/.cache/pixel-52220DLAQ001FC.lock test/e2e/pixel-f-rows.sh HUB_BIN OUT_DIR
#   (-o: the lock's fd is not inherited, so a hub this script starts never holds the phone's lock; its hubs are found by their home, whatever the binary is called)
# Taps are in the 2000-wide frame screenshots are shown in (x1.21 to the screen). A check passes from the hub's events.jsonl or the stand-in's log, or from changed pixels on the viewer; never from the driver's say-so.
# Never touches the iPhone; only the gpui app package; uses `adb reverse` for the hub's port and removes it when it ends.
set -u; HUB=$(realpath "$1"); O=$2; PORT=${PORT:-3893}; S=52220DLAQ001FC; PKG=dev.backplane.mobile.gpui; HERE=$(cd "$(dirname "$0")" && pwd)
A() { adb -s $S "$@"; }; shot() { A exec-out screencap -p > "$O/shots/$1.png"; }
T() { A shell input tap $(python3 -c "print(int($1*1.21), int($2*1.21))"); }
res=(); ok() { echo "$1 $2 $3"; res+=("{\"id\":\"$2\",\"ok\":$( [ "$1" = ok ] && echo true || echo false),\"ev\":\"$3\"}"); }
rm -rf "$O"; mkdir -p "$O/shots"; (bun "$HERE/pixel-hub.mjs" "$HUB" "$O/hub" $PORT > "$O/hub.out" 2>&1 &); sleep 8
EV=$O/hub/home/events.jsonl; F=$O/hub/fake.log; HP=$(python3 -c "import json;print(json.load(open('$O/hub/state.json'))['pid'])")
cleanup() { A reverse --remove tcp:$PORT >/dev/null 2>&1; kill $HP $(pgrep -f -- "--home $O/hub/home") 2>/dev/null; pkill -f "pixel-hub.mjs $HUB $O/hub" 2>/dev/null; echo "{\"rows\":[$(IFS=,; echo "${res[*]}")]}" > "$O/rows.json"; }; trap cleanup EXIT
A reverse tcp:$PORT tcp:$PORT >/dev/null; A shell am force-stop $PKG
INV=$(python3 -c "import json,urllib.parse;print(urllib.parse.quote(json.load(open('$O/hub/state.json'))['invite'],safe=''))")
A shell am start -W -a android.intent.action.VIEW -d "bpgpui://pair?url=$INV" $PKG >/dev/null; sleep 6; shot pair-prompt; T 301 560; sleep 8; shot sidebar
T 170 603; sleep 3                       # Thread 1 (approval-required)
say() { T 1000 670; sleep 0.7; A shell input text "$1"; sleep 0.7; A shell input keyevent KEYCODE_BACK; sleep 1; T 1907 779; }
wait_for() { for i in $(seq 1 ${2:-25}); do eval "$1" && return 0; sleep 1; done; return 1; }
say 'zqapprove%snow'; wait_for "[ \$(grep -c AskOpened $EV) -ge 1 ]"; sleep 2; shot approval; T 500 303; wait_for "grep -q 'PERM Bash.*allow' $F"
grep -q 'PERM Bash.*allow' $F && ok ok F06-phone-allow "the approval card on the phone: Allow tapped, the agent got allow" || ok FAIL F06-phone-allow "no allow reached the agent"
sleep 4; say 'zqapprove%sagain'; wait_for "[ \$(grep -c AskOpened $EV) -ge 2 ]"; sleep 2; T 500 503; wait_for "grep -q 'PERM Bash.*deny' $F"
grep -q 'PERM Bash.*deny' $F && ok ok F06-phone-decline "Decline tapped, the agent got deny" || ok FAIL F06-phone-decline "no deny reached the agent"
sleep 3; n=$(grep -c '"state":"running"' $EV); say 'a%slong%sturn'; wait_for "[ \$(grep -c '\"state\":\"running\"' $EV) -gt $n ]"; sleep 2; shot running; T 1907 779
wait_for "grep -q '\"state\":\"interrupted\"' $EV" && ok ok F07-phone-stop "Stop tapped on a running turn: TurnChanged interrupted in the hub log" || ok FAIL F07-phone-stop "not interrupted"
sleep 3; T 1835 104; sleep 4; D=$(python3 -c "print(' '.join(str(int(v*1.21)) for v in (700,500,1300,560)))"); views=""
for v in "Board 70 211" "Schematic 250 211" "3D 403 211" "Mech 660 211"; do set -- $v; T $2 $3; for k in 1 2 3 4 5 6 7 8; do sleep 5; A shell input swipe 1210 680 1212 682 60; sleep 1; shot v-$1; [ "$(python3 -W ignore -c "
from PIL import Image
g=Image.open('$O/shots/v-$1.png').convert('RGB').crop((0,330,2424,1000)).convert('L');print(sum(1 for p in g.getdata() if p>60)>6000)")" = True ] && break; done; A shell input swipe $D 500; sleep 2; shot v-$1-moved
  c=$(python3 -W ignore -c "
from PIL import Image,ImageChops
h=lambda n:Image.open('$O/shots/%s.png'%n).convert('RGB').crop((0,330,2424,1000))
a=h('v-$1');b=h('v-$1-moved');g=a.convert('L');print(('drawn' if sum(1 for p in g.getdata() if p>60)>6000 else 'blank')+('+moved' if ImageChops.difference(a,b).getbbox() else '+still'))"); views="$views $1:$c"; done
case "$views" in *blank*|*still*) ok FAIL F17-phone "viewer pages on the phone:$views";; *) ok ok F17-phone "viewer pages on the phone (drawn, and a swipe changed each):$views";; esac
T 60 104; sleep 3; say 'zqask'; wait_for "grep -q '\"kind\":\"input\"' $EV"; sleep 3; shot question; T 135 503; wait_for "grep -q 'PERM AskUserQuestion.*Barrel' $F"
grep -q 'PERM AskUserQuestion.*Barrel' $F && ok ok F06-phone-question "after Stop, the next turn's question is drawn on the phone (shots/question.png: Which port? with USB-C and Barrel) and Barrel tapped reaches the agent" || ok FAIL F06-phone-question "no answer reached the agent after the question"
sleep 2; kill $HP; sleep 6; shot offline; pkill -f -- "--home $O/hub/home"; sleep 1
(cd $O/hub/proj && BACKPLANE_AGENTS=$O/hub/bin PATH=$O/hub/bin:$PATH BACKPLANE_NO_UPDATE=1 SHELL=/bin/sh FAKE_LOG=$F HOME=$O/hub/home nohup "$HUB" --home $O/hub/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $O/hub2.log 2>&1 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&- &); sleep 20
n=$(grep -c "after the reconnect" $EV); say 'zqshort%safter%sthe%sreconnect'; wait_for "[ \$(grep -c 'after the reconnect' $EV) -gt $n ]" 30 && ok ok F21-phone "the hub was killed (the phone showed its offline banner, shots/offline.png) and started again on the same home: a message sent from the phone afterwards reached the new hub process" || ok FAIL F21-phone "no message reached the restarted hub"
shot final
# leave the app as the next lane finds it: unpaired, at the pairing screen
A shell am start -W -a android.intent.action.VIEW -d "bpgpui://unpair" $PKG >/dev/null; sleep 4; shot unpaired
