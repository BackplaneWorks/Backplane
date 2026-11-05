#!/bin/bash
# Run the Rust hub + GPUI desktop beside today's Backplane, on a private COPY of a store, on its own port and home.
#
#   scripts/gpui/switch.sh start [--no-client]   fresh copy, hub (+ policy helper), then the GPUI client
#   scripts/gpui/switch.sh stop                  stop what start made; the copy stays (to look at, or to start again on)
#   scripts/gpui/switch.sh status                what runs, memory of each process, counts only
#   scripts/gpui/switch.sh rollback              stop and delete the copy. There is nothing to undo: today's app never stopped.
#
# What it never does: touch the live hub (3787/3788), ~/.backplane-bend, ~/.local/share/backplane, or any tailnet. The copy is
# made by reading the source file; nothing is written to the source. Agents are stand-ins only (no model, no credentials: the hub
# runs with a fake HOME and BACKPLANE_COPY=1, which makes every outside-reaching action a no-op, docs/gpui/COPY-GUARD.md).
#
# Environment (all optional):
#   SWITCH_SOURCE   the store to copy: a folder holding events.jsonl. Default ~/.backplane-gpui-copy (the verified copy).
#                   `live` reads ~/.backplane-bend/events.jsonl (read-only; the person's call, see SWITCHING BACK below).
#   SWITCH_PORT     default 3790 (3787, 3788, 3799, 3821 are refused: the live hub, its dev hub, the shared test hub)
#   SWITCH_DIR      state: home, logs, pids. Default ~/.backplane-gpui-switch (mode 700; never under the repo)
#   SWITCH_MEM      the scope's MemoryMax, default 3700M (swap is always 0). Never raise it to make a run fit; report instead.
#   SWITCH_DISPLAY  where the client opens. Default $DISPLAY; there is no default screen on a headless run: use :61 (build/gpui/xvfb).
#   HUB_BIN POLICY_BIN CLIENT_BIN  by absolute path; default the main checkout's build/gpui/{hub/prov/backplane-hub-full,
#                   slim/bp-policy-bp,linux/backplane-gpui}
#
# SWITCHING BACK. Starting this changes nothing of today's app, so going back is `rollback` and nothing else. What is lost is
# whatever was done on the copy: threads, messages, settings and project edits made in the GPUI app live in the copy's own log,
# not in today's. Nothing from the copy flows back into ~/.backplane-bend, and nothing done in today's app after the copy was
# made shows up here (a start makes a fresh copy every time).
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
HERE=$(cd "$(dirname "$0")/../.." && pwd)
SW=${SWITCH_DIR:-$HOME/.backplane-gpui-switch}
PORT=${SWITCH_PORT:-3790}
MEM=${SWITCH_MEM:-3700M}
SRC=${SWITCH_SOURCE:-$HOME/.backplane-gpui-copy}
HUB_BIN=${HUB_BIN:-$ROOT/build/gpui/hub/prov/backplane-hub-full}
POLICY_BIN=${POLICY_BIN:-$ROOT/build/gpui/slim/bp-policy-bp}
CLIENT_BIN=${CLIENT_BIN:-$ROOT/build/gpui/linux/backplane-gpui}
DISP=${SWITCH_DISPLAY:-${DISPLAY:-}}
H=$SW/home

die() { echo "switch: $*" >&2; exit 1; }
alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }
pidof_file() { [ -f "$SW/$1.pid" ] && cat "$SW/$1.pid" || true; }
rss_mb() { awk '/^VmRSS/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
hwm_mb() { awk '/^VmHWM/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
helper_pid() { local h; h=$(pidof_file hub); alive "$h" && pgrep -P "$h" 2>/dev/null | head -1 || true; }

guards() {
  case "$PORT" in 3787|3788|3799|3821) die "port $PORT belongs to another hub";; esac
  case "$PORT" in ''|*[!0-9]*) die "bad port";; esac
  case "$SW" in "$HOME/.backplane-bend"*|"$HOME/.local/share/backplane"*|"$ROOT"*|"$HERE"*) die "state dir $SW is not allowed there";; esac
  [ -x "$HUB_BIN" ] || die "no hub binary $HUB_BIN"
  [ -x "$POLICY_BIN" ] || die "no policy helper $POLICY_BIN"
}

source_file() {
  if [ "$SRC" = live ]; then echo "$HOME/.backplane-bend/events.jsonl"; else echo "$SRC/events.jsonl"; fi
}

cmd_stop() {
  local who p
  for who in client hub; do
    p=$(pidof_file "$who")
    if alive "$p"; then kill "$p" 2>/dev/null || true; fi
  done
  # the helper is the hub's child and ends with it (--parent-pid); wait for both to go, then leave no pid files
  local i=0
  while [ $i -lt 20 ]; do
    alive "$(pidof_file hub)" || alive "$(pidof_file client)" || break
    sleep 0.5; i=$((i+1))
  done
  for who in client hub; do alive "$(pidof_file $who)" && die "$who ($(pidof_file $who)) did not stop; look at it, I kill nothing else"; rm -f "$SW/$who.pid"; done
  echo "stopped. The copy is kept in $H (rollback deletes it)."
}

cmd_start() {
  guards
  mkdir -p "$SW"; chmod 700 "$SW"
  alive "$(pidof_file hub)" && die "already running (pid $(pidof_file hub)); stop first"
  local src; src=$(source_file); [ -f "$src" ] || die "no $src"
  [ -z "$(ss -ltn "sport = :$PORT" 2>/dev/null | tail -n +2)" ] || die "port $PORT is in use"
  # disk: the copy must leave 20 GB free
  local need free; need=$(( $(stat -c %s "$src") / 1048576 )); free=$(df -Pm / | awk 'NR==2{print $4}')
  [ $((free - need)) -ge 20480 ] || die "copy needs ${need} MB and / has ${free} MB free: under 20 GB would remain; tell the coordinator"
  # a fresh private copy: only the log (no tokens, no secrets, no vault); the source is only read
  rm -rf "$H"; mkdir -p "$H" "$SW/bin" "$SW/fakehome/.cache" "$SW/proj"; chmod 700 "$H"
  cp --reflink=auto "$src" "$H/events.jsonl"; chmod 600 "$H/events.jsonl"
  echo "copied $(wc -l < "$H/events.jsonl") events ($need MB) from $([ "$SRC" = live ] && echo 'the live store (read only)' || echo "$SRC")"
  # stand-in agents: nothing may call a model
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$HERE/test/tools/claude_standin.ts" > "$SW/bin/claude"; chmod +x "$SW/bin/claude"
  local n; for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$SW/bin/$n"; chmod +x "$SW/bin/$n"; done
  # the hub in its own capped scope, a fake HOME (no real credentials), copy guard on, loopback only
  ( cd "$SW/proj" && exec env -i PATH="$PATH" LC_ALL=C XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}" HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" \
      BACKPLANE_COPY=1 BACKPLANE_AGENTS="$SW/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream \
      systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
      "$HUB_BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground --policy-bin "$POLICY_BIN" \
      > "$SW/hub.log" 2>&1 < /dev/null ) &
  echo $! > "$SW/hub.pid"
  local hp; hp=$(cat "$SW/hub.pid") i=0
  # before anything else: the stand-ins and the copy guard are in the hub's own environment
  sleep 1
  alive "$hp" || { tail -3 "$SW/hub.log"; die "the hub exited at once"; }
  tr '\0' '\n' < "/proc/$hp/environ" | grep -qx "BACKPLANE_AGENTS=$SW/bin" && tr '\0' '\n' < "/proc/$hp/environ" | grep -qx "BACKPLANE_COPY=1" \
    || { kill "$hp" 2>/dev/null; rm -f "$SW/hub.pid"; die "hub $hp has no BACKPLANE_AGENTS/BACKPLANE_COPY=1 in its environment: stopped"; }
  # a big log takes minutes to fold; wait for /hello, bounded
  echo "hub pid $hp, environment checked (stand-ins, copy guard). Waiting for it to open the log..."
  while [ $i -lt "${SWITCH_WAIT:-1800}" ]; do
    alive "$hp" || { tail -5 "$SW/hub.log"; die "the hub exited while opening (see $SW/hub.log)"; }
    curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" && break
    sleep 1; i=$((i+1))
  done
  curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" || die "no /hello after ${i}s; hub still running as pid $hp, see $SW/hub.log"
  echo "hub answers on http://127.0.0.1:$PORT after ${i}s"
  if [ "${1:-}" != "--no-client" ]; then
    [ -x "$CLIENT_BIN" ] || die "no client $CLIENT_BIN (use --no-client to run the hub alone)"
    [ -n "$DISP" ] || die "no display: set SWITCH_DISPLAY (:61 is the isolated one) or pass --no-client"
    ( exec env DISPLAY="$DISP" ${VK_ICD_FILENAMES:+VK_ICD_FILENAMES="$VK_ICD_FILENAMES"} HOME="$HOME" \
        systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
        "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$H/token" > "$SW/client.log" 2>&1 < /dev/null ) &
    echo $! > "$SW/client.pid"; echo "client pid $(cat "$SW/client.pid") on $DISP"
  fi
  echo "Running beside today's Backplane (untouched on 3787). To go back: scripts/gpui/switch.sh rollback"
}

cmd_status() {
  local hp cp pp; hp=$(pidof_file hub); cp=$(pidof_file client); pp=$(helper_pid)
  if alive "$hp"; then
    echo "hub    pid $hp  rss $(rss_mb "$hp") MB (peak $(hwm_mb "$hp"))"
    [ -n "$pp" ] && echo "helper pid $pp  rss $(rss_mb "$pp") MB (peak $(hwm_mb "$pp"))"
    alive "$cp" && echo "client pid $cp  rss $(rss_mb "$cp") MB"
    echo "port   $PORT: $(curl -s -m3 -o /dev/null -w '/hello %{http_code}' "http://127.0.0.1:$PORT/hello" || echo 'no answer')"
    [ -f "$H/events.jsonl" ] && echo "copy   $(wc -l < "$H/events.jsonl") events, $(du -sm "$H" | cut -f1) MB"
  else
    echo "not running"; [ -d "$H" ] && echo "a copy is kept in $H"
  fi
  echo "today's Backplane (3787): $(curl -s -m3 -o /dev/null -w '/hello %{http_code}' http://127.0.0.1:3787/hello || echo 'no answer') (read only probe)"
}

cmd_rollback() {
  [ -d "$SW" ] && cmd_stop || true
  [ -d "$H" ] && rm -rf "$H" && echo "copy deleted."
  echo "Nothing to undo: today's app (3787, ~/.backplane-bend) was never stopped or changed."
  echo "Lost by going back: everything done on the copy (threads, messages, settings, edits), since it lived only in the copy."
}

case "${1:-}" in
  start) shift; cmd_start "$@";;
  stop) cmd_stop;;
  status) cmd_status;;
  rollback) cmd_rollback;;
  *) sed -n '2,10p' "$0"; exit 2;;
esac
