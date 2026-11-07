#!/bin/sh
# Real-hub check of the GPUI Files tab: the Rust hub (HUB_BIN) alone, no helper, on a temp home with a project of mixed files, stand-in agents only, loopback,
# isolated Xvfb display, everything in one memory-capped scope. The client is driven by test/files/files-script.json (steps `open_viewer`,
# `files`, `files_download`, `wait`), screenshots are taken by the client itself (`files: shot:PATH`).
# usage: desktop-files-run.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTNAME   ->  build/gpui/shots/files/OUTNAME-*.png, build/gpui/results/files/OUTNAME.*
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); MAIN=${GPUI_MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
CLIENT=$1; HUBBIN=$2; ASSETS=$3; N=${4:-files}
R=$ME/build/gpui/files-run-$N; OUT=$ME/build/gpui/results/files; SH=$ME/build/gpui/shots/files
X=$MAIN/build/gpui/xvfb
PORT=${GPUI_PORT:-3841}; DISP=${GPUI_DISP:-:63}
mkdir -p $OUT $SH
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] || { echo "need CLIENT_BIN and HUB_BIN"; exit 2; }
[ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP is in use (set GPUI_DISP=:6N)"; exit 2; }
rm -rf $R; mkdir -p $R/home $R/proj/docs/deep $R/proj/mech $R/bin $R/fakehome/.cache $R/downloads $R/up
FIX=$ME/test/files/fixtures
cp $FIX/plot/ecc83-pp.kicad_pcb $FIX/plot/ecc83-pp.kicad_sch $R/proj/; echo '{}' > $R/proj/ecc83-pp.kicad_pro; cp $FIX/mesh/bracket.step $R/proj/mech/
printf '# Notes\n\nThe board is the ecc83 preamp.\n\n## Todo\n- check the layout\n' > $R/proj/notes.md
printf 'plain text file\nsecond line\n' > $R/proj/readme.txt; printf 'alpha\n' > $R/proj/docs/a.txt; printf 'deep file\n' > $R/proj/docs/deep/b.txt
for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do printf 'row %s\n' $i > $R/proj/log-$i.log; done
convert -size 320x200 gradient:'#2a6' '#26a' -gravity center -pointsize 28 -annotate 0 'pic.png' $R/proj/pic.png 2>/dev/null || convert -size 320x200 xc:teal $R/proj/pic.png
convert -size 400x300 xc:white -gravity center -pointsize 30 -annotate 0 'PDF PAGE ONE' $R/proj/doc.pdf 2>/dev/null || true
printf 'uploaded through the drop zone\n' > $R/up/dropped.txt
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > $R/bin/claude; chmod +x $R/bin/claude
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; NOW=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"files","root":R+"/proj","at":NOW},{"$":"ThreadCreated","id":"t1","project":"p1","title":"files thread","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":NOW}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
sed -e "s#@SHOTS@#$SH/$N#g" -e "s#@UP@#$R/up/dropped.txt#g" $ME/test/files/files-${FILES_XINPUT:+xinput-}script.json > $R/script.json
export LC_ALL=C
# FILES_XINPUT=1: the client only opens the Files tab; real X input (files-xinput.sh) does the rest
XIN=; [ -n "${FILES_XINPUT:-}" ] && XIN="sleep 8; DISPLAY=$DISP R=$R SH=$SH N=$N sh $ME/test/files/files-xinput.sh; kill \$(cat $R/client.pid)"
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- nice -n 19 sh -c '
set -eu
cleanup() { for f in client i3 xvfb hub; do [ -f '$R'/$f.pid ] && kill $(cat '$R'/$f.pid) 2>/dev/null || true; done; sleep 1; }
trap cleanup EXIT INT TERM
export BACKPLANE_AGENTS='$R'/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream HOME='$R'/fakehome XDG_CACHE_HOME='$R'/fakehome/.cache XDG_CONFIG_HOME='$R'/fakehome/.config XDG_DATA_HOME='$R'/fakehome/.local
cd '$R'/proj
'$HUBBIN' --home '$R'/home --port '$PORT' --host 127.0.0.1 --no-tailscale --foreground > '$R'/hub.log 2>&1 &
echo $! > '$R'/hub.pid; sleep 5
HP=$(cat '$R'/hub.pid)
echo "hub cmdline: $(tr "\0" " " < /proc/$HP/cmdline)"
tr "\0" "\n" < /proc/$HP/environ | grep -q "^BACKPLANE_AGENTS='$R'/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; kill $HP; exit 3; }
LD_LIBRARY_PATH='$X'/usr/lib/x86_64-linux-gnu '$X'/usr/bin/Xvfb '$DISP' -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > '$R'/xvfb.log 2>&1 &
echo $! > '$R'/xvfb.pid; sleep 2
printf "font pango:monospace 8\nnew_window none\nfor_window [class=\".*\"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket '$R'/i3.sock\n" > '$R'/i3.conf
mkdir -p -m 700 '$R'/run
XDG_RUNTIME_DIR='$R'/run DISPLAY='$DISP' i3 -c '$R'/i3.conf > '$R'/i3.log 2>&1 &
echo $! > '$R'/i3.pid; sleep 1
export DISPLAY='$DISP' VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS='$ASSETS' BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_DOWNLOADS='$R'/downloads
'$CLIENT' --url http://127.0.0.1:'$PORT' --token-file '$R'/home/token --script '$R'/script.json --exit-after 120 > '$OUT'/'$N'.client.log 2>&1 &
echo $! > '$R'/client.pid
'"$XIN"'
wait $(cat '$R'/client.pid) || true
'
echo "--- files metrics"; grep -E 'GPUI_METRIC .*"event":"(files|files_download|view)"' $OUT/$N.client.log | cut -c1-260
echo "--- project after"; (cd $R/proj && find . -type f -newer $R/script.json -o -type d -name newdir -o -name 'renamed*' -o -name dropped.txt | sort | head -20); echo "--- downloads"; ls -la $R/downloads
