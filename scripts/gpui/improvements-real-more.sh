#!/bin/sh
# The readiness run's other two real-model checks for Improvements (the person's go for real turns on throwaway homes and scratch repos, never a store copy):
#   writer   the REPORT AGENT against a real model: description and evidence in, "Write report with agent", wait, "Load agent report": the report is the
#            agent's answer to the report prompt (template v1, the original description verbatim).
#   edit     a handed-off turn that makes a small REAL edit through APPROVAL (the hand-off thread runs approval-required; the harness answers each ask with its
#            primary button) and commits in its own worktree; then, in the scratch repository only, a scratch copy of the land.sh flow: the guard refuses a direct
#            move of its `gpui`, land.sh refuses an unreviewed range, a scratch reviewer records its OK, land.sh moves the scratch `gpui`. The real gpui is never touched.
# Same safety as improvements-real-turn.sh: a throwaway Backplane home and a scratch repository under build/gpui/imp-real-<mode>/, no BACKPLANE_AGENTS and no
# BACKPLANE_COPY (refused if present), the hub's --home checked, the isolated display :61, one scope capped at 3700M with swap 0. HOME is the person's own (claude's sign-in).
# usage: improvements-real-more.sh writer|edit CLIENT_BIN HUB_BIN ASSETS_DIR
set -eu
MODE=$1; CLIENT=$2; HUBBIN=$3; ASSETS=$4
case "$MODE" in writer|edit) ;; *) echo "mode: writer or edit"; exit 2 ;; esac
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}; HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/../.." && pwd)
R=$ROOT/build/gpui/imp-real-$MODE; OUT=$ROOT/build/gpui/results/improvements-real-$MODE; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; PORT=3836; DISP=:61
mkdir -p $OUT $SH
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] || { echo "need the client and hub binaries"; exit 2; }
[ ! -e /tmp/.X11-unix/X61 ] || { echo "display :61 is in use"; exit 2; }
command -v claude >/dev/null || { echo "claude not on PATH"; exit 2; }
rm -rf $R; mkdir -p $R/home $R/saved; rm -f $SH/improvements-real-$MODE-*.png
P=$R/proj/backplane-gpui; mkdir -p $P/scripts/gpui $P/build/gpui
# the scratch lock stands in for scripts/bp-lock.sh (no shared lock is taken); the scratch repository carries copies of the landing scripts
printf '#!/bin/sh\nshift; [ "$1" = -- ] && shift; exec "$@"\n' > $R/scratch-lock.sh; chmod +x $R/scratch-lock.sh
if [ "$MODE" = edit ]; then
  for f in land.sh review-ok.sh ref-guard.hook install-guard.sh; do cp $HERE/$f $P/scripts/gpui/$f; done
  printf 'Scratch repository for a Backplane hand-off test. Edit freely; nothing here is real.\n' > $P/AGENTS.md; cp $P/AGENTS.md $P/build/gpui/COMMON.md
fi
echo scratch > $P/README.md
( cd $P && git init -q && git config user.email t@t && git config user.name t && git add -A && git commit -qm init && git branch gpui && git checkout -q gpui )
[ "$MODE" = edit ] && ( cd $P && sh scripts/gpui/install-guard.sh )
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
open(R+"/home/events.jsonl","w").write(json.dumps({"$":"ProjectCreated","id":"p1","title":"Backplane (scratch)","root":R+"/proj/backplane-gpui","at":NOW},separators=(",",":"))+"\n")
PY
export LC_ALL=C
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
cleanup() { for f in client i3 xvfb hub; do [ -f '$R'/$f.pid ] && kill $(cat '$R'/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
R='$R'; ROOT='$ROOT'; REPO='$REPO'; X='$X'; HUBBIN='$HUBBIN'; CLIENT='$CLIENT'; ASSETS='$ASSETS'; PORT='$PORT'; DISP='$DISP'; OUT='$OUT'; SH='$SH'; MODE='$MODE'; P='$P'
unset BACKPLANE_AGENTS BACKPLANE_COPY BACKPLANE_SIDE_BY_SIDE
export BACKPLANE_NO_UPDATE=1
cd $P
$HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --policy-bin $ROOT/build/gpui/slim/bp-policy-page --window-bin $ROOT/build/gpui/hub/new/window > $R/hub.log 2>&1 &
echo $! > $R/hub.pid; sleep 5
HP=$(cat $R/hub.pid)
if tr "\0" "\n" < /proc/$HP/environ | grep -q "^BACKPLANE_AGENTS=\|^BACKPLANE_COPY="; then echo "stand-in or copy env present: stopping"; kill $HP; exit 3; fi
tr "\0" " " < /proc/$HP/cmdline | grep -q -- "--home $R/home " || { echo "hub home is not the throwaway one: stopping"; kill $HP; exit 3; }
echo "hub pid $HP: real agents, home $R/home"; curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
export BACKPLANE_GPUI_OPEN=/bin/true BACKPLANE_GPUI_SAVE_DIR=$R/saved BACKPLANE_GPUI_SHOTS=$SH
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $REPO/test/gpui/improvements-real-$MODE.json --exit-after 220 > $OUT/client.log 2>&1 &
echo $! > $R/client.pid
wait $(cat $R/client.pid) || true
cp $R/hub.log $OUT/hub.log; cp $R/home/events.jsonl $OUT/events.jsonl 2>/dev/null || true
'
python3 $HERE/improvements-real-more-check.py $MODE $OUT $R $ROOT
