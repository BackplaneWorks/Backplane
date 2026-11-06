#!/bin/bash
# Time a hub's start on a COPY of a log, under a memory cap, with stand-in agents and the copy guard on. Prints the hub's own phase lines,
# the time to /hello and the idle RSS. The home is yours to throw away: pass a folder holding a copy of events.jsonl (never the live store).
#   scripts/gpui/startup-bench.sh HUB_BIN HOME_COPY [PORT=3799] [CAP=3700M]
set -u
HUB=${1:?HUB_BIN}; H=${2:?HOME_COPY}; PORT=${3:-3799}; CAP=${4:-3700M}
case "$H" in "$HOME/.backplane-bend"*|"$HOME/.backplane-gpui") echo "refusing: $H looks like a live home"; exit 2;; esac
SW=$(mktemp -d /tmp/bp-startup-bench-XXXX); mkdir -p "$SW/bin" "$SW/fakehome" "$SW/proj"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$SW/bin/$n"; chmod +x "$SW/bin/$n"; done
rm -f "$H/hub.lock"; t0=$(date +%s.%N)
( cd "$SW/proj" && exec setsid env -i PATH="$PATH" LC_ALL=C XDG_RUNTIME_DIR="/run/user/$(id -u)" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus" \
    HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" BACKPLANE_COPY=1 BACKPLANE_AGENTS="$SW/bin" BACKPLANE_NO_UPDATE=1 BACKPLANE_PASSIVE=1 BP_STANDIN_MODE=stream \
    systemd-run --user --scope -q -p "MemoryMax=$CAP" -p MemorySwapMax=0 -- env -u DBUS_SESSION_BUS_ADDRESS -u XDG_RUNTIME_DIR "$HUB" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$SW/hub.log" 2>&1 < /dev/null ) &
for i in $(seq 1 1200); do curl -sf -m1 "http://127.0.0.1:$PORT/hello" > /dev/null 2>&1 && { echo "/hello after $(echo "$(date +%s.%N) - $t0" | bc) s (poll step 0.05 s, includes the cap's scope start)"; break; }; sleep 0.05; done
sleep 5; pid=$(cut -d' ' -f1 "$H/hub.lock" 2>/dev/null)
grep -E '^startup:' "$SW/hub.log"; [ -z "$pid" ] || { grep VmRSS "/proc/$pid/status"; kill "$pid"; }
echo "log: $SW/hub.log"
