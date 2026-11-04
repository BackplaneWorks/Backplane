#!/bin/sh
# A hub of our own for the Android checks (stand-in agent, no model): tailnet IP only, own scope.
# usage: android-hub.sh start | stop
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); H=$HOME/bp-gpui-android-hub; PORT=${PORT:-3814}; IP=100.64.84.41
case "$1" in
start)
  mkdir -p $H/bin $H/home $H/proj
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > $H/bin/claude; chmod +x $H/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $H/bin/$n; chmod +x $H/bin/$n; done
  echo hello > $H/proj/README.md
  BACKPLANE_AGENTS=$H/bin BACKPLANE_NO_UPDATE=1 BACKPLANE_LOG=debug BP_STANDIN_MODE=stream BP_STREAM_N=12 BP_STREAM_MS=60 \
  cd $H/proj; nohup systemd-run --user --scope -q -p MemoryMax=4G -p MemorySwapMax=0 \
    "$ROOT/build/gpui/backplane-serve" --home $H/home --port $PORT --host $IP --no-tailscale --foreground --web "$ROOT/build/gpui/web" > $H/hub.log 2>&1 &
  echo $! > $H/pid; sleep 6; tail -3 $H/hub.log; curl -s -m5 http://$IP:$PORT/hello | head -c 200; echo ;;
stop) p=$(cat $H/pid); kill $p 2>/dev/null; sleep 2; rm -f $H/pid; echo stopped ;;
esac
