#!/bin/bash
# Two-hub GPUI rig for "every send cuts in" (PARITY row 37). Pair after `up`/`hubs`: (cd test/e2e/pairing && python3 ../interrupt/pair.py $PWD $BP_TMP/r2),
# then `two.sh hub B` so B pulls A's threads. Stand-in logs: $R/standin-A.log, standin-B.log (USER/NOW lines). Every process is its own user unit (bpi-*), so each can be stopped and started alone:
#   two.sh up|hubs     hubs A (owner of the far thread) and B (the window's), relays, Xvfb, i3, the window on B
#   two.sh hub A|B     (re)start one hub          two.sh relay A|B on|off   (A: B->A link; B: window->B link)
#   two.sh down        stop everything
set -u
RS=$(cd "$(dirname "$0")" && pwd); SRC=${SRC:-$(cd "$RS/../../.." && pwd)}; . "$SRC/scripts/lib-bptmp.sh"; R=${R:-$BP_TMP/r2}
MAIN=${MAIN:-$(dirname "$(git -C "$SRC" rev-parse --path-format=absolute --git-common-dir)")}
HUB=${HUB:-$SRC/native/host/target/fast/backplane-hub}; CLIENT=${CLIENT:-$SRC/native/gpui/target/fast/backplane-gpui}; X=${X:-$MAIN/build/gpui/xvfb}
PA=3881 PB=3882 RA=3891 RB=3892 DN=153
BUN=${BUN:-$(command -v bun || echo $HOME/.bun/bin/bun)}
unit() { local n=$1; shift; systemctl --user stop bpi-$n 2>/dev/null; systemctl --user reset-failed bpi-$n 2>/dev/null; systemd-run --user --unit=bpi-$n -q --collect -p MemoryMax=4G "$@"; }
henv() { echo "-E BACKPLANE_AGENTS=$R/bin -E BACKPLANE_NO_UPDATE=1 -E BP_STANDIN_MODE=stream -E BP_STREAM_N=${N:-700} -E BP_STREAM_MS=150 -E FAKE_LOG=$R/standin-$1.log -E HOME=$R/fh$1 -E XDG_CACHE_HOME=$R/fh$1/.cache -E XDG_CONFIG_HOME=$R/fh$1/.config -E XDG_DATA_HOME=$R/fh$1/.local -E BACKPLANE_LOCK_DIR=$R -E BACKPLANE_NAME=rig-$1 -E BACKPLANE_PEERS=$([ $1 = A ] && echo http://127.0.0.1:3882 || echo http://127.0.0.1:3891) -E BACKPLANE_URL=$([ $1 = A ] && echo http://127.0.0.1:3891 || echo http://127.0.0.1:3882) -E LC_ALL=C"; }
hub() { local w=$1 p; [ $w = A ] && p=$PA || p=$PB; unit hub$w $(henv $w) --working-directory=$R/proj$w $HUB --home $R/home$w --port $p --host 127.0.0.1 --no-tailscale --foreground; }
relay() { local w=$1 lp tp; [ $w = A ] && { lp=$RA; tp=$PA; } || { lp=$RB; tp=$PB; }; if [ "$2" = on ]; then unit relay$w /usr/bin/python3 -I $RS/relay.py $lp $tp; else systemctl --user stop bpi-relay$w; fi; }
seed() { python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
base=lambda: [{"$":"SettingSet","key":k,"value":v,"at":N} for k,v in (("restart.continue","off"),("onboard.done","1"),("onboard.uber","1"))]
a=base()+[{"$":"ProjectCreated","id":"pa","title":"far-lamp","root":R+"/projA","at":N},{"$":"ThreadCreated","id":"ta","project":"pa","title":"Far work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N}]
b=base()+[{"$":"ProjectCreated","id":"pb","title":"lamp","root":R+"/projB","at":N},
  {"$":"ThreadCreated","id":"tb","project":"pb","title":"Local work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},
  {"$":"ThreadCreated","id":"th","project":"pb","title":"Helper","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N+1}]
for w,ev in (("A",a),("B",b)): open(f"{R}/home{w}/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
json.dump([{"wait":3000000}],open(R+"/script.json","w"))
PY
}
case "${1:-}" in
up|hubs)
  for d in $(ls /tmp/.X11-unix/ 2>/dev/null); do [ "$d" = X$DN ] && { echo "display :$DN in use"; exit 2; }; done
  rm -rf $R; mkdir -p $R/homeA $R/homeB $R/projA $R/projB $R/bin $R/fhA/.cache $R/fhB/.cache $R/fhC/.cache $R/shots
  for p in projA projB; do (cd $R/$p && git init -q -b main && echo hi > README.md && git add -A && git -c user.email=r@x -c user.name=rig commit -qm init); done
  printf '#!/bin/sh\npython3 -I -c %s "$@" >> %s 2>/dev/null\nexec %s %s "$@"\n' "'import json,sys;a=sys.argv[1:];print(json.loads(a[a.index(\"--mcp-config\")+1])[\"mcpServers\"][\"backplane\"][\"url\"])'" "$R/mcp.urls" $BUN $SRC/test/tools/claude_standin.ts > $R/bin/claude; chmod +x $R/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
  seed; hub A; hub B; relay A on; relay B on; sleep 5
  for p in $PA $PB; do curl -s -m3 -o /dev/null -w "hello $p %{http_code}\n" http://127.0.0.1:$p/hello; done
  [ "$1" = hubs ] && { echo "hubs up (no window)"; exit 0; }
  unit xvfb -E LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu $X/usr/bin/Xvfb :$DN -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb; sleep 2
  printf "ipc-socket $R/i3.sock\nfont pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\n" > $R/i3.conf
  unit i3 -E DISPLAY=:$DN i3 -c $R/i3.conf; sleep 1
  $0 client; echo up ;;
client)
  unit client -E DISPLAY=:$DN -E HOME=$R/fhC -E XDG_CONFIG_HOME=$R/fhC/.config -E XDG_DATA_HOME=$R/fhC/.local -E XDG_CACHE_HOME=$R/fhC/.cache -E VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json -E LP_NUM_THREADS=2 -E LIBGL_ALWAYS_SOFTWARE=1 -E GALLIUM_DRIVER=llvmpipe -E BACKPLANE_GPUI_ASSETS=$SRC/native/gpui/assets -E BACKPLANE_GPUI_RECTS=1 -E LC_ALL=C \
    bash -c "exec $CLIENT --url http://127.0.0.1:$RB --token-file $R/homeB/token --script $R/script.json --exit-after 6000 --theme dark >> $R/client.log 2>&1" ;;
hub) hub $2 ;;
relay) relay $2 $3 ;;
down) for n in client i3 xvfb relayA relayB hubA hubB; do systemctl --user stop bpi-$n 2>/dev/null; done ;;
esac
