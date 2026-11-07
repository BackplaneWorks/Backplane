#!/bin/sh
# Improvements, end to end, on a temp home (never the store copy): the GPUI client on the Rust hub, stand-in agents only (no model), loopback,
# the isolated display :61 (never the person's screen), one scope capped at 3700M with swap 0, nice 19. The run (test/improvements.json):
# open the panel, type the description, evidence and details, capture this window (ffmpeg x11grab of our own window id), select the screenshot,
# diagnostics and details, review the public issue, open the GitHub draft (a recorder stands in for the browser: nothing leaves the machine),
# write the report with an agent (stand-in), load it, record the window, play it, then hand the request to an agent thread (internal).
# Pictures: build/gpui/shots/desktop/improvements-NN-*.png (this window only). Checks: improvements-e2e-check.py on the log, the events and the files.
# usage: improvements-e2e.sh CLIENT_BIN HUB_BIN ASSETS_DIR            (binaries come from a build job, never built here)
set -eu
ROOT=${GPUI_MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/.." && pwd)
CLIENT=$1; HUBBIN=$2; ASSETS=$3
R=$ROOT/build/gpui/imp-e2e; OUT=$ROOT/build/gpui/results/improvements; SH=$ROOT/build/gpui/shots/desktop
X=$ROOT/build/gpui/xvfb; PORT=3834; DISP=${IMP_DISPLAY:-:61}
mkdir -p $OUT $SH
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] || { echo "need the client and hub binaries"; exit 2; }
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP is in use"; exit 2; }
rm -rf $R; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache $R/saved; rm -f $SH/improvements-*.png
# stand-in agents: nothing may call a model
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$REPO/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
# the opener stands in for the browser and the player: it records what it was asked to open
printf '#!/bin/sh\necho "$1" >> %s/opened.txt\n' "$R" > $R/bin/open; chmod +x $R/bin/open
# the Backplane source project: a throwaway repository named backplane-gpui with a local main branch; and a second project
mkdir -p $R/proj/backplane-gpui $R/proj/board
( cd $R/proj/backplane-gpui && git init -q && git config user.email t@t && git config user.name t && echo x > README.md && git add . && git commit -qm init && git branch -M main )
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"Backplane","root":R+"/proj/backplane-gpui","at":NOW},{"$":"ProjectCreated","id":"p2","title":"Board","root":R+"/proj/board","at":NOW},
    {"$":"ThreadCreated","id":"t1","project":"p2","title":"A board thread","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
export LC_ALL=C
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
cleanup() { for f in client i3 xvfb hub; do [ -f '$R'/$f.pid ] && kill $(cat '$R'/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
R='$R'; ROOT='$ROOT'; REPO='$REPO'; X='$X'; HUBBIN='$HUBBIN'; CLIENT='$CLIENT'; ASSETS='$ASSETS'; PORT='$PORT'; DISP='$DISP'; OUT='$OUT'; SH='$SH'
export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local
cd $R/proj/backplane-gpui
$HUBBIN --home $R/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --window-bin $ROOT/build/gpui/hub/new/window > $R/hub.log 2>&1 &
echo $! > $R/hub.pid; sleep 5
HP=$(cat $R/hub.pid)
# the hub must run with the stand-in agents before any turn
tr "\0" "\n" < /proc/$HP/environ | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub (pid $HP): stopping"; kill $HP; exit 3; }
echo "hub pid $HP environ ok: $(tr "\0" "\n" < /proc/$HP/environ | grep "^BACKPLANE_AGENTS=")"; curl -s -m3 -o /dev/null -w "hello %{http_code}\n" http://127.0.0.1:$PORT/hello
LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb $DISP -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > $R/xvfb.log 2>&1 &
echo $! > $R/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
DISPLAY=$DISP i3 -c $R/i3.conf > $R/i3.log 2>&1 &
echo $! > $R/i3.pid; sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS BACKPLANE_GPUI_RECTS=1
export BACKPLANE_GPUI_OPEN=$R/bin/open BACKPLANE_GPUI_SAVE_DIR=$R/saved BACKPLANE_GPUI_SHOTS=$SH
$CLIENT --url http://127.0.0.1:$PORT --token-file $R/home/token --script $REPO/test/improvements.json --exit-after 90 > $OUT/client.log 2>&1 &
echo $! > $R/client.pid
wait $(cat $R/client.pid) || true
cp $R/hub.log $OUT/hub.log; cp $R/home/events.jsonl $OUT/events.jsonl 2>/dev/null || true
'
cp $R/opened.txt $OUT/opened.txt 2>/dev/null || true
ls -R $R/home/attachments > $OUT/attachments.txt 2>/dev/null || true
python3 $HERE/improvements-e2e-check.py $OUT $R $SH
