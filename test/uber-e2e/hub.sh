#!/bin/bash
# hub.sh start|stop|term|status RUN [BIN]: the throwaway hub of an uberbot run, in its own capped scope, on a fake HOME, stand-in agents only.
set -eu
cmd=$1; R=$2; BIN=${3:-$R/backplane-hub}; PORT=${E2E_PORT:-3811}; H=$R/home
case "$(realpath -m "$R")" in "$HOME/.backplane"*|"$HOME/.local/share/backplane"*) echo "refused: $R"; exit 1;; esac
pid() { cut -d' ' -f1 "$H/hub.lock" 2>/dev/null || true; }
case "$cmd" in
start)
  mkdir -p "$H" "$R/fakehome" "$R/bubdir" "$R/ag"; install -m755 "$(dirname "$0")/standin-claude.py" "$R/ag/claude"
  for n in codex grok muse opencode pi omp agy; do printf '#!/bin/sh\nexit 1\n' > "$R/ag/$n"; chmod 755 "$R/ag/$n"; done
  setsid env -i HOME="$R/fakehome" PATH=/usr/bin:/bin USER=h TZ=Europe/Helsinki BACKPLANE_AGENTS="$R/ag" E2E_RUN="$R" BACKPLANE_NO_UPDATE=1 BACKPLANE_PEERS= BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}" \
    systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -E BACKPLANE_AGENTS="$R/ag" -- "$BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$R/hub.log" 2>&1 < /dev/null &
  for i in $(seq 1 300); do curl -sf -m1 "http://127.0.0.1:$PORT/hello" >/dev/null 2>&1 && break; sleep 0.1; done
  p=$(pid); [ -n "$p" ] || { echo "no hub"; tail -5 "$R/hub.log" | cut -c1-200; exit 1; }
  tr '\0' '\n' < /proc/$p/environ | grep -qx "BACKPLANE_AGENTS=$R/ag" || { kill "$p"; echo "NOT stand-in only: stopped"; exit 1; }
  tr '\0' '\n' < /proc/$p/environ | grep -qx "HOME=$R/fakehome" || { kill "$p"; echo "real HOME: stopped"; exit 1; }
  echo "hub pid $p port $PORT, stand-ins verified from /proc/$p/environ";;
stop|term) p=$(pid); [ -n "$p" ] && kill -TERM "$p" 2>/dev/null || true; for i in $(seq 1 100); do [ -n "$p" ] && kill -0 "$p" 2>/dev/null || break; sleep 0.1; done; kill -0 "$p" 2>/dev/null && echo "still running" || echo "stopped $p";;
status) p=$(pid); kill -0 "$p" 2>/dev/null && echo "running $p" || echo "down";;
esac
