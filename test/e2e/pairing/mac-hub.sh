#!/bin/sh
# mac-hub.sh start|stop|restart|expire HOME PORT HOST: a temp-home hub of the pairing rig on macOS. HUB names the binary, AGENTS a folder of stand-in agents.
# `expire` stops the hub, moves every `expiresAt` in HOME/devices-v1.json into the past (the hub has no test clock) and starts it again.
set -u
cmd=$1; home=$2; port=$3; host=$4
pidf="$home.pid"
up() { for _ in $(seq 1 100); do curl -s -m1 -o /dev/null "http://$host:$port/hello" && return 0; sleep 0.2; done; return 1; }
case $cmd in
  start) mkdir -p "$home"; BACKPLANE_AGENTS="${AGENTS:-$home/agents}" BACKPLANE_NO_UPDATE=1 nohup "$HUB" --home "$home" --port "$port" --host "$host" --no-tailscale --foreground > "$home.log" 2>&1 < /dev/null & echo $! > "$pidf"; up ;;
  stop) [ -f "$pidf" ] && kill "$(cat "$pidf")" 2>/dev/null; for _ in $(seq 1 50); do kill -0 "$(cat "$pidf" 2>/dev/null)" 2>/dev/null || break; sleep 0.2; done ;;
  restart) "$0" stop "$home" "$port" "$host"; "$0" start "$home" "$port" "$host" ;;
  expire) "$0" stop "$home" "$port" "$host"
    python3 - "$home/devices-v1.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p)); n = [0]
def walk(o):
    if isinstance(o, dict):
        for k, v in list(o.items()):
            if k == "expiresAt": o[k] = 1; n[0] += 1
            else: walk(v)
    elif isinstance(o, list):
        for v in o: walk(v)
walk(d); json.dump(d, open(p, "w")); print("expiresAt moved back in %d place(s)" % n[0])
PY
    "$0" start "$home" "$port" "$host" ;;
esac
