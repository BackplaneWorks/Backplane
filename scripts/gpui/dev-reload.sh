#!/bin/bash
# Follow the test builds: on each new one that dev-cut.sh publishes, relaunch on the SAME copy. The window comes back on the same thread at the same place
# with the same drafts (native/gpui/src/screens/place.rs). docs/gpui/DEV-RELOAD.md.
#
# Start it through `side-by-side.sh watch` (a real copy: it asks your yes first), or directly on a seeded `try` home. It stays in the foreground.
#   A build whose hub is unchanged swaps only the window, at once.
#   A build with a new hub waits until no turn is running: a restart would cut it, and a copy resumes nothing. Enter relaunches anyway.
#   A build that does not come up is marked bad, what ran before is put back, and the next cut is tried.
#
# Environment: GPUI_DEV_DIR (default ~/.backplane-gpui-dev), GPUI_RELOAD_POLL (seconds, default 2); switch.sh's SWITCH_DIR, SWITCH_PORT, SWITCH_DISPLAY.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}
SW=${SWITCH_DIR:-$HOME/.backplane-gpui-switch}
PORT=${SWITCH_PORT:-3790}
POLL=${GPUI_RELOAD_POLL:-2}
TITLE='Backplane GPUI'
export SWITCH_DIR=$SW
say() { printf '%s %s\n' "$(date +%H:%M:%S)" "$*"; }
tell() { say "$*"; notify-send -a Backplane "$TITLE" "$*" 2>/dev/null || true; }
cut_now() { head -1 "$DEV/CURRENT" 2>/dev/null || true; }
hub_up() { kill -0 "$(cat "$SW/hub.pid" 2>/dev/null)" 2>/dev/null; }
# a number, or ? when this hub cannot say (a build from before /dev/turns). The token goes on stdin, never on a command line.
turns() {
  local t r; t=$(cat "$SW/home/token" 2>/dev/null) || { echo '?'; return; }
  r=$(printf 'Authorization: Bearer %s\n' "$t" | curl -s -m3 -H @- "http://127.0.0.1:$PORT/dev/turns" 2>/dev/null) || { echo '?'; return; }
  case "$r" in '{"running":'*'}') r=${r#*:}; echo "${r%\}}";; *) echo '?';; esac
}
in_use() { cat "$SW/$1.bin" 2>/dev/null; }
hub_same() { local b; b=$(in_use hub); [ -n "$b" ] && [ -f "$b" ] && cmp -s "$b" "$DEV/builds/$1/backplane-hub" && hub_up; }
# switch.sh keeps every check a start makes; its output stays on this terminal (on a real copy it refuses without one)
reload() {  # reload HUB CLIENT ASSETS COMMIT WHAT
  env HUB_BIN="$1" CLIENT_BIN="$2" ${3:+BACKPLANE_GPUI_ASSETS="$3"} ${4:+BACKPLANE_COMMIT="$4"} "$HERE/switch.sh" reload "$5"
}
# the window has drawn the thread list: the client's own `usable` line within 60 s
window_up() {
  local i=0
  while [ $i -lt 120 ]; do
    grep -q '"event":"usable"' "$SW/client.log" 2>/dev/null && return 0
    kill -0 "$(cat "$SW/client.pid" 2>/dev/null)" 2>/dev/null || return 1
    sleep 0.5; i=$((i+1))
  done
  return 1
}
# i3: a new window maps on the focused workspace. It goes back to the one the old window was on, found by the X id that is new.
i3_windows() {
  command -v i3-msg >/dev/null || return 0
  DISPLAY=${SWITCH_DISPLAY:-${DISPLAY:-}} i3-msg -t get_tree 2>/dev/null | python3 -I -c '
import json,sys
def walk(n,ws):
    if n.get("type")=="workspace": ws=n.get("name")
    p=n.get("window_properties") or {}
    if n.get("window") and (p.get("title") or "").startswith(sys.argv[1]): print(n["window"],ws)
    for c in n.get("nodes",[])+n.get("floating_nodes",[]): walk(c,ws)
try: walk(json.load(sys.stdin),None)
except ValueError: pass' "$TITLE"
}
i3_back() {  # i3_back BEFORE: BEFORE is what i3_windows said before the swap
  local ws id i=0
  ws=$(printf '%s\n' "$1" | awk 'NF>=2{print $2; exit}'); [ -n "$ws" ] || return 0
  while [ $i -lt 20 ]; do
    id=$(i3_windows | awk 'NF>=2{print $1}' | grep -vxF -f <(printf '%s\n' "$1" | awk '{print $1}') | head -1)
    if [ -n "$id" ]; then DISPLAY=${SWITCH_DISPLAY:-${DISPLAY:-}} i3-msg "[id=\"$id\"] move container to workspace \"$ws\"" >/dev/null 2>&1; return 0; fi
    sleep 0.25; i=$((i+1))
  done
}

apply() {  # apply BUILD WHAT
  local b=$DEV/builds/$1 what=$2 ph pc win commit a=
  ph=$(in_use hub); pc=$(in_use client); win=$(i3_windows)
  commit=$(head -1 "$b/COMMIT" 2>/dev/null || true); [ ! -d "$b/assets" ] || a=$b/assets
  say "relaunching on $1 ($what)"
  if reload "$b/backplane-hub" "$b/backplane-gpui" "$a" "$commit" "$what" && window_up; then
    i3_back "$win"; printf '%s\n' "$1" > "$DEV/.shown"; tell "Now running $1"; return 0
  fi
  printf '%s\n' "$1" > "$DEV/.bad"; tell "Build $1 did not come up; going back. See $SW/hub.log and $SW/client.log"
  [ -n "$ph" ] && [ -n "$pc" ] && [ -x "$ph" ] && [ -x "$pc" ] || { say "nothing to go back to"; return 1; }
  if reload "$ph" "$pc" "${BACKPLANE_GPUI_ASSETS:-}" "" both && window_up; then i3_back "$win"; say "back on the build before"
  else tell "Going back failed too: the app is down. Logs are in $SW"; fi
}

[ -f "$SW/.switch-owned" ] && [ -f "$SW/home/events.jsonl" ] || { echo "dev-reload: no copy in $SW: start one first (side-by-side.sh, or switch.sh try)" >&2; exit 1; }
trap 'echo; say "stopped following; the app keeps running"; exit 0' INT TERM
waiting=
say "following $DEV/CURRENT (running: $(in_use client))"
while :; do
  c=$(cut_now); bad=$(cat "$DEV/.bad" 2>/dev/null || true); shown=$(cat "$DEV/.shown" 2>/dev/null || true); force=
  if [ -z "$c" ] || [ "$c" = "$shown" ] || [ "$c" = "$bad" ] || [ ! -d "$DEV/builds/$c" ]; then waiting=; sleep "$POLL"; continue; fi
  what=both; if hub_same "$c"; then what=client; fi
  if [ "$what" = both ] && hub_up; then
    n=$(turns)
    if [ "$n" != 0 ]; then
      case "$n" in '?') why="this hub cannot say whether a turn is running";; 1) why="1 turn is running";; *) why="$n turns are running";; esac
      [ "$waiting" = "$c:$n" ] || tell "Build $c is ready; $why. Waiting (Enter here relaunches now and stops them)."
      waiting=$c:$n
      if [ -t 0 ]; then if read -r -t "$POLL" _; then force=1; fi; else sleep "$POLL"; fi
      [ -n "$force" ] || continue
    fi
  fi
  waiting=; apply "$c" "$what"
done
