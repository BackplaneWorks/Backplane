#!/bin/bash
# Hot reload for fresh mode (scripts/gpui/fresh.sh, ~/.backplane-gpui-fresh): follow the builds dev-cut.sh publishes and relaunch the fresh hub and window on each.
#   scripts/gpui/fresh-watch.sh            stay in the foreground (Ctrl-C stops following; the app keeps running)
# Same hub binary: only the window is reopened, at once. A new hub waits until no turn is running (a restart cuts a turn); Enter in a terminal relaunches anyway.
# The window comes back where it was (gpui-place.json, gpui-drafts.json in the home). A build whose hub does not answer is marked bad and the build before is put back.
# Env: FRESH_DIR, FRESH_PORT (as fresh.sh), GPUI_DEV_DIR, GPUI_RELOAD_POLL (s, default 2), GPUI_RELOAD_BOOT (s the hub may take to answer, default 120).
set -u
# The hub reaps every process that carries its BACKPLANE_AGENT_OWNER mark and is not its child (agents/proc.rs reap_orphans). A watcher started from an agent turn inherits the mark,
# and the first hub restart it makes would kill it, and the window it is about to open. Run again without the mark.
if [ -n "${BACKPLANE_AGENT_OWNER:-}" ]; then exec env -u BACKPLANE_AGENT_OWNER "$0" "$@"; fi
HERE=$(cd "$(dirname "$0")" && pwd)
D=${FRESH_DIR:-$HOME/.backplane-gpui-fresh}; PORT=${FRESH_PORT:-3797}; H=$D/home
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; POLL=${GPUI_RELOAD_POLL:-2}; BOOT=${GPUI_RELOAD_BOOT:-120}
ST=${BACKPLANE_DEV_STATUS:-$DEV/STATUS}
status() { printf '{"phase":"%s","text":"%s","done":0,"total":0%s}\n' "$1" "$2" "${3:-}" > "$ST.w" && mv -f "$ST.w" "$ST"; }
REQ=$DEV/RESTART   # the window's Restart now (two clicks) leaves this file: it is a press of Enter
clear_status() { rm -f "$ST" "$ST.w"; }
say() { printf '%s %s\n' "$(date +%H:%M:%S)" "$*"; }
tell() { say "$*"; notify-send -a Backplane 'Backplane GPUI (fresh)' "$*" 2>/dev/null || true; }
alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }
pid() { cat "$D/$1.pid" 2>/dev/null || true; }
cur() { head -1 "$DEV/CURRENT" 2>/dev/null || true; }
running_dir() { local p; p=$(pid hub); alive "$p" && dirname "$(readlink "/proc/$p/exe" 2>/dev/null)" || true; }
hello() { printf 'Authorization: Bearer %s\n' "$(cat "$H/token" 2>/dev/null)" | curl -sf -m3 -H @- "http://127.0.0.1:$PORT/hello" >/dev/null 2>&1; }
# a number, or ? when the hub cannot say
turns() {
  local r; r=$(printf 'Authorization: Bearer %s\n' "$(cat "$H/token" 2>/dev/null)" | curl -s -m3 -H @- "http://127.0.0.1:$PORT/dev/turns" 2>/dev/null) || { echo '?'; return; }
  case "$r" in '{"running":'*'}') r=${r#*:}; echo "${r%\}}";; *) echo '?';; esac
}
go() {  # go BUILD_DIR WHAT(client|both): relaunch on that build
  local b=$1 p
  export BIN=$b FRESH_TAILNET_PORT=$(cat "$D/tailnet.port" 2>/dev/null || true)
  [ -n "$FRESH_TAILNET_PORT" ] || unset FRESH_TAILNET_PORT
  if [ "$2" = client ]; then p=$(pid client); alive "$p" && kill "$p"; rm -f "$D/client.pid"; sleep 1
  else p=$(pid hub); "$HERE/fresh.sh" stop >/dev/null; local i=0; while alive "$p" && [ $i -lt 100 ]; do sleep 0.2; i=$((i+1)); done; fi
  "$HERE/fresh.sh" start || return 1
  local i=0; while [ $i -lt "$BOOT" ]; do hello && return 0; alive "$(pid hub)" || return 1; sleep 1; i=$((i+1)); done; return 1
}
trap 'echo; say "stopped following; the app keeps running"; exit 0' INT TERM
[ -d "$H" ] || { echo "fresh-watch: no fresh home in $H: run fresh.sh start first" >&2; exit 1; }
say "following $DEV/CURRENT (running: $(basename "$(running_dir)"))"
rm -f "$REQ"; waiting=
while :; do
  c=$(cur); now=$(basename "$(running_dir)"); bad=$(cat "$DEV/.fresh-bad" 2>/dev/null || true)
  if [ -z "$c" ] || [ "$c" = "$now" ] || [ "$c" = "$bad" ] || [ ! -d "$DEV/builds/$c" ]; then waiting=; sleep "$POLL"; continue; fi
  what=both; cmp -s "$(running_dir)/backplane-hub" "$DEV/builds/$c/backplane-hub" 2>/dev/null && what=client
  if [ "$what" = both ] && alive "$(pid hub)"; then
    n=$(turns)
    if [ "$n" != 0 ]; then
      [ "$waiting" = "$c:$n" ] || tell "Build $c is ready; waiting for $n running turn(s) to finish (Enter relaunches now)."
      waiting=$c:$n; status waiting "Build ${c##*-} ready: waiting for $n running turn(s) to end" ",\"action\":\"restart\",\"turns\":$n"
      if [ -e "$REQ" ]; then rm -f "$REQ"; say "relaunch asked for from the window"
      elif [ -t 0 ]; then read -r -t "$POLL" _ || continue
      else sleep "$POLL"; continue; fi
    fi
  fi
  waiting=; prev=$(running_dir); say "relaunching on $c ($what)"; status relaunching "Relaunching on build ${c##*-}"
  if go "$DEV/builds/$c" "$what"; then clear_status; tell "Now running $c"
  else
    clear_status; printf '%s\n' "$c" > "$DEV/.fresh-bad"; tell "Build $c did not come up; going back. See $D/hub.log"
    [ -n "$prev" ] && [ -x "$prev/backplane-hub" ] && go "$prev" both && say "back on $(basename "$prev")" || tell "Going back failed: the app is down. Logs are in $D"
  fi
done
