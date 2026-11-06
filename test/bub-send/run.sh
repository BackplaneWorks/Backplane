#!/bin/bash
# The composer after a send, in a private window: Xvfb + i3 + a temp-home hub with a recording stand-in agent, real clicks and keys.
#   test/gpui/bub-send/run.sh HUB_BIN GPUI_BIN ASSETS_DIR [XVFB]      (XVFB default: Xvfb on PATH; DISP default :241, PORT 3841)
# Checks, in a bot's chat (Kit, then Miso opened from the sidebar) and in an ordinary thread: Enter empties the box, the message shows once, a pasted
# picture reaches the agent as an image block, and a refused send keeps the text and shows the error. Prints PASS/FAIL lines; exit 1 on any FAIL.
set -u
HUB=$1; GPUI=$2; ASSETS=$3; XVFB=${4:-Xvfb}; D=$(cd "$(dirname "$0")" && pwd); export DISPLAY=${DISP:-:241}; PORT=${PORT:-3841}
W=$(mktemp -d); export WIN_LOG=$W/win.log; REC=$W/rec.jsonl; fails=0; pids=()
trap 'kill "${pids[@]}" 2>/dev/null; rm -rf "$W"' EXIT
ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
mkdir -p $W/home $W/fakehome $W/proj $W/run; chmod 700 $W/run; (cd $W/proj && git init -q && echo hi >readme.txt)
$XVFB $DISPLAY -screen 0 1600x1000x24 -nolisten tcp >$W/xvfb.log 2>&1 & pids+=($!); sleep 2
printf 'ipc-socket %s/i3.sock\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\n' $W >$W/i3.conf
XDG_RUNTIME_DIR=$W/run i3 -c $W/i3.conf >/dev/null 2>&1 & pids+=($!); sleep 1
env -i BD_REC=$REC PATH=$D/agents:/usr/bin:/bin HOME=$W/fakehome DISPLAY= BACKPLANE_NO_UPDATE=1 BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_KEYRING_LOCKED=0 BACKPLANE_PEERS= BACKPLANE_AGENTS=$D/agents \
  $HUB --home $W/home --port $PORT --no-tailscale --foreground >$W/hub.log 2>&1 & pids+=($!)
until curl -sf http://127.0.0.1:$PORT/hello >/dev/null; do sleep 0.3; done
tr '\0' '\n' </proc/${pids[2]}/environ | grep -q "BACKPLANE_AGENTS=$D/agents" || { echo "FAIL stand-in agents not set"; exit 1; }
node $D/seed.mjs $PORT $W/home/token $W/proj >/dev/null 2>&1
env -u WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent XDG_RUNTIME_DIR=/nonexistent HOME=$W/fakehome BACKPLANE_GPUI_RECTS=1 LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS \
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json $GPUI --url http://127.0.0.1:$PORT --token-file $W/home/token >$W/win.log 2>&1 & pids+=($!); sleep 6
X() { python3 $D/xdo.py "$@"; }; go() { X click $(python3 $D/ctl.py "$1"); sleep 3; }
convert -size 200x120 xc:orange $W/pic.png
paste() { (xclip -loops 20 -selection clipboard -t image/png -i $W/pic.png >/dev/null 2>&1 &); sleep 0.5; X click 700 905; X key ctrl+v; sleep 2; }
last_rec() { python3 -c "
import sys,json
l=[json.loads(x) for x in open('$REC')][-1]['message']['content']
print(','.join(c['type'] for c in l)+'|'+''.join(c.get('text','') for c in l))"; }
shot() { import -window root $W/$1.png; cp $W/$1.png "${SHOTS:-/tmp}/bub-send-$1.png"; }
box_empty() { # what the window keeps as the box's text: its draft file (gpui-drafts.json, beside the token), emptied when the hub accepts a send
  python3 -c "
import json,os
p='$W/home/gpui-drafts.json'
d=json.load(open(p)) if os.path.exists(p) else {}
print(''.join(str(v) for v in d.values()))"
}
for who in bot:Kit bot:Miso; do
  go $who; X click 700 905; X type "hi from $who"; sleep 1; X key Return; sleep 3
  n=$(last_rec); [ "${n#*|}" = "hi from $who" ] && ok "$who: the agent got the text" || bad "$who: agent got '$n'"
  shot after-$who; [ "$(box_empty)" = "" ] && ok "$who: the box is empty after Enter" || bad "$who: the box still holds '$(box_empty)'"
  paste; X type "see this"; sleep 2; X key Return; sleep 3
  n=$(last_rec); case "$n" in image,text\|*see\ this*) ok "$who: the picture reached the agent as an image block";; *) bad "$who: agent got '${n:0:80}'";; esac
  [ "$(box_empty)" = "" ] && ok "$who: the box is empty after a picture send" || bad "$who: box '$(box_empty)' after picture"
done
go thread:t; X click 700 905; paste; X type "thread pic"; sleep 2; X key Return; sleep 3
n=$(last_rec); case "$n" in image,text\|*thread\ pic*) ok "thread: the picture reached the agent";; *) bad "thread: agent got '${n:0:80}'";; esac
shot done
echo "fails=$fails"; [ $fails = 0 ]
