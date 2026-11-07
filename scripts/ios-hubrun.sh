#!/bin/sh
# on h@backplane: a hub of our own for the iOS checks, stand-in agents only. usage: run.sh start|stop
# (the hub binary is ~/bp-gpui-ios-hub/hub; the folder is deleted by the caller)
set -eu
H=$HOME/bp-gpui-ios-hub; BUN=$HOME/.bun/bin/bun; IP=100.96.192.111; PORT=3811
case "$1" in
start)
  mkdir -p $H/home $H/proj $H/bin
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$BUN" "$H/tools/claude_standin.ts" > $H/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $H/bin/$n; done; chmod +x $H/bin/*
  [ -d $H/proj/.git ] || git -C $H/proj init -q
  export BACKPLANE_AGENTS=$H/bin BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=${BP_STANDIN_MODE:-plain} TZ=Etc/UTC
  cd $H/proj
  nohup systemd-run --user --scope -q \
    $H/hub --home $H/home --port $PORT --host $IP --no-tailscale --foreground > $H/hub.log 2>&1 &
  sleep 10
  P=$(ss -ltnp 2>/dev/null | grep ":$PORT " | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2)
  echo $P > $H/pid; echo "pid $P"; tr '\0' '\n' < /proc/$P/environ | grep BACKPLANE_AGENTS;;
stop) P=$(cat $H/pid); kill $P; sleep 2; ss -ltn | grep -c ":$PORT " || true;;
esac
