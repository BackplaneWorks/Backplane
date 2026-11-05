#!/bin/sh
# Our own hub for the desktop checks: stand-in agents (no model), loopback only, own scope.
# usage: desktop-hub.sh start | stop     (PORT default 3821, home build/gpui/desktop-hub/home)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); H=$ROOT/build/gpui/desktop-hub; PORT=${PORT:-3821}
case "$1" in
start)
  mkdir -p $H/bin $H/home $H/proj
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $H/bin/claude; chmod +x $H/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $H/bin/$n; chmod +x $H/bin/$n; done
  [ -f $H/proj/README.md ] || echo "# demo project" > $H/proj/README.md
  export BACKPLANE_AGENTS=$H/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream BP_STREAM_N=6 BP_STREAM_MS=60
  cd $H/proj; nohup systemd-run --user --scope -q -p MemoryMax=4G -p MemorySwapMax=0 \
    "$ROOT/build/gpui/backplane-serve" --home $H/home --port $PORT --host 127.0.0.1 --no-tailscale --foreground --web "$ROOT/build/gpui/web" > $H/hub.log 2>&1 &
  echo $! > $H/pid; sleep 6; tail -2 $H/hub.log; curl -s -m5 http://127.0.0.1:$PORT/hello | head -c 120; echo ;;
stop) p=$(cat $H/pid); kill $p 2>/dev/null; sleep 2; rm -f $H/pid; echo stopped ;;
esac
