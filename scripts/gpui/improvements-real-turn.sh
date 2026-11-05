#!/bin/sh
# ONE real Claude turn through the Improvements hand-off (the person's go, 2026-11-05): the GPUI client on the Rust hub, a throwaway Backplane home and
# a scratch git repository named backplane-gpui (with a local gpui branch) under build/gpui/imp-real/, loopback, the isolated display :61, one scope capped at
# 3700M with swap 0. NO stand-in folder and NO copy mode: the hub starts the real `claude` (the person's own sign-in, HOME as it is). The request is
# harmless (reply with one sentence; no edits, no tools). Nothing is pushed; nothing outside the scratch folder is written by the agent's task.
# usage: improvements-real-turn.sh CLIENT_BIN HUB_BIN ASSETS_DIR
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/../.." && pwd)
CLIENT=$1; HUBBIN=$2; ASSETS=$3
R=$ROOT/build/gpui/imp-real; OUT=$ROOT/build/gpui/results/improvements-real; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; PORT=3835; DISP=:61
mkdir -p $OUT $SH
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] || { echo "need the client and hub binaries"; exit 2; }
[ ! -e /tmp/.X11-unix/X61 ] || { echo "display :61 is in use"; exit 2; }
command -v claude >/dev/null || { echo "claude not on PATH"; exit 2; }
rm -rf $R; mkdir -p $R/home $R/saved; rm -f $SH/improvements-real-*.png
mkdir -p $R/proj/backplane-gpui
( cd $R/proj/backplane-gpui && git init -q && git config user.email t@t && git config user.name t && echo "scratch" > README.md && git add . && git commit -qm init && git branch gpui )
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"Backplane (scratch)","root":R+"/proj/backplane-gpui","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
export LC_ALL=C
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
cleanup() { for f in client i3 xvfb hub; do [ -f '$R'/$f.pid ] && kill $(cat '$R'/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
R='$R'; ROOT='$ROOT'; REPO='$REPO'; X='$X'; HUBBIN='$HUBBIN'; CLIENT='$CLIENT'; ASSETS='$ASSETS'; PORT='$PORT'; DISP='$DISP'; OUT='$OUT'; SH='$SH'
unset BACKPLANE_AGENTS BACKPLANE_COPY BACKPLANE_SIDE_BY_SIDE
export BACKPLANE_NO_UPDATE=1
cd $R/proj/backplane-gpui
$HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground > $R/hub.log 2>&1 &
echo $! > $R/hub.pid; sleep 5
HP=$(cat $R/hub.pid)
# this run is the one with a REAL agent: the hub must have no stand-in folder and no copy mode, and its home must be the throwaway one
if tr "\0" "\n" < /proc/$HP/environ | grep -q "^BACKPLANE_AGENTS=\|^BACKPLANE_COPY="; then echo "stand-in or copy env present: stopping"; kill $HP; exit 3; fi
tr "\0" "\0" < /proc/$HP/cmdline | tr "\0" " " | grep -q -- "--home $R/home " || { echo "hub home is not the throwaway one: stopping"; kill $HP; exit 3; }
echo "hub pid $HP: real agents, home $R/home"; curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
export BACKPLANE_GPUI_OPEN=/bin/true BACKPLANE_GPUI_SAVE_DIR=$R/saved BACKPLANE_GPUI_SHOTS=$SH
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $REPO/test/gpui/improvements-real.json --exit-after 140 > $OUT/client.log 2>&1 &
echo $! > $R/client.pid
wait $(cat $R/client.pid) || true
cp $R/hub.log $OUT/hub.log; cp $R/home/events.jsonl $OUT/events.jsonl 2>/dev/null || true
cd $R/proj/backplane-gpui && git status --short > $OUT/scratch-status.txt; git worktree list > $OUT/scratch-worktrees.txt; git branch --list > $OUT/scratch-branches.txt
'
python3 - $OUT <<'PY'
import json,sys
out=sys.argv[1]; ev=[]
for l in open(out+"/events.jsonl",errors="replace"):
    try: ev.append(json.loads(l))
    except ValueError: pass
th=[e for e in ev if e.get("$")=="ThreadCreated" and str(e.get("id","")).startswith("improvement-")]
tid=th[0]["id"] if th else ""
print("thread:",tid)
print("worktree:",[ (e.get("branch"),e.get("worktree","")[-40:]) for e in ev if e.get("$")=="WorktreeSet" and e.get("id")==tid])
print("turns:",[e.get("state") for e in ev if e.get("$")=="TurnChanged" and e.get("thread")==tid])
for e in ev:
    if e.get("thread")==tid and e.get("$")=="MessagePosted" and e.get("role")=="assistant": print("assistant:",e.get("text","")[:400])
    if e.get("thread")==tid and e.get("$")=="TurnUsage": print("usage:",{k:e.get(k) for k in e if k not in ("$","thread","at")})
    if e.get("thread")==tid and e.get("$")=="ActivityLogged" and e.get("tone") in ("error","warn"): print("note:",e.get("kind"),str(e.get("summary"))[:200])
print("scratch status:",open(out+"/scratch-status.txt").read().strip() or "(clean)")
print("branches:",open(out+"/scratch-branches.txt").read().split())
PY
