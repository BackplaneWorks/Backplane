#!/bin/bash
# dev reload end to end, on a private X display and a seeded synthetic home, with stand-in agents: nothing real is read and nothing reaches your screen.
#   test/gpui/switch/reload_e2e.sh BUILD_DIR [DISPLAY_NUMBER=83] [PORT=3834]
# BUILD_DIR holds backplane-hub, backplane-gpui and assets/. Prints PASS or FAIL per check; exit 1 on any FAIL. Stops only the pids it started.
set -u
B=$(cd "${1:?usage: reload_e2e.sh BUILD_DIR}" && pwd); N=${2:-83}; PORT=${3:-3834}
HERE=$(cd "$(dirname "$0")/../../.." && pwd)
T=$(mktemp -d /tmp/bp-reload-XXXXXX); SW=$T/sw; DEV=$T/dev; D=:$N
XV=${XVFB_ROOT:-$HOME/.local/xvfb-root}
[ ! -e "/tmp/.X11-unix/X$N" ] || { echo "display $D exists: pick another"; exit 2; }
export SWITCH_DIR=$SW SWITCH_PORT=$PORT SWITCH_DISPLAY=$D GPUI_DEV_DIR=$DEV GPUI_RELOAD_POLL=1 SWITCH_NO_POLICY=1 SWITCH_WAIT=60
export HUB_BIN=$B/backplane-hub CLIENT_BIN=$B/backplane-gpui BACKPLANE_GPUI_ASSETS=$B/assets
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json BACKPLANE_GPUI_PLACE=1 BACKPLANE_GPUI_HOLD=1 SWITCH_LOGS=$T/logs
fails=0; ok() { echo "PASS $*"; }; no() { echo "FAIL $*"; fails=$((fails+1)); }
check() { local w=$1; shift; if "$@"; then ok "$w"; else no "$w"; fi; }
wait_for() { local n=$1 i=0; shift; while [ $i -lt $((n*4)) ]; do "$@" && return 0; sleep 0.25; i=$((i+1)); done; return 1; }
pid() { cat "$SW/$1.pid" 2>/dev/null; }
log_has() { grep -q "$1" "$SW/$2" 2>/dev/null; }
place() { python3 -I -c 'import json,sys; v=json.load(open(sys.argv[1])); print(v["target"].get("id",""))' "$SW/home/gpui-place.json" 2>/dev/null; }
token_turns() { printf 'Authorization: Bearer %s\n' "$(cat "$SW/home/token")" | curl -s -m3 -H @- "http://127.0.0.1:$PORT/dev/turns"; }
shown() { [ "$(cat "$DEV/.shown" 2>/dev/null)" = "$1" ]; }
cut_build() { "$HERE/scripts/gpui/dev-cut.sh" "$1" | awk '{print $2}'; }
variant() { local v=$T/$1; mkdir -p "$v"; cp "$B/backplane-hub" "$B/backplane-gpui" "$v/"; cp -r "$B/assets" "$v/"; printf '\n#%s' "$1" >> "$v/backplane-hub"; echo "$v"; }
XP=; IP=; WP=
finish() {
  trap - EXIT INT TERM
  [ -z "$WP" ] || kill "$WP" 2>/dev/null
  "$HERE/scripts/gpui/switch.sh" stop > /dev/null 2>&1
  [ -z "$IP" ] || kill "$IP" 2>/dev/null; [ -z "$XP" ] || kill "$XP" 2>/dev/null
  echo "logs: $T"
}
trap finish EXIT INT TERM
( LD_LIBRARY_PATH=$XV/usr/lib/x86_64-linux-gnu exec "$XV/usr/bin/Xvfb" "$D" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb ) > /dev/null 2>&1 < /dev/null & XP=$!
sleep 1.5
printf '# empty\n' > "$T/i3.conf"
( DISPLAY=$D HOME=$T exec i3 -c "$T/i3.conf" ) > /dev/null 2>&1 < /dev/null & IP=$!
sleep 1

# 1. the first run: open the 4th thread, leave a draft in it
echo '[{"select":3},{"wait":1500},{"type":"replies:30 grow this thread"},{"send":true},{"wait":9000},{"scroll":-300},{"wait":400},{"scroll_state":"left"},{"type":"a draft kept across builds"},{"wait":1500}]' > "$T/first.json"
echo '[{"wait":1500},{"scroll_state":"back"}]' > "$T/back.json"
SWITCH_CLIENT_ARGS="--script $T/first.json" "$HERE/scripts/gpui/switch.sh" try || { no "try started"; exit 1; }
wait_for 30 log_has '"script_done"' client.log; check "the first client ran its script" log_has '"script_done"' client.log
wait_for 5 test -n "$(place)"; T1=$(place); check "the place file names the thread ($T1)" test -n "$T1"
wait_for 3 grep -q 'a draft kept' "$SW/home/gpui-drafts.json"
check "the place file is private" test "$(stat -c %a "$SW/home/gpui-place.json")" = 600
check "no draft text in the place file" sh -c "! grep -q 'a draft kept' '$SW/home/gpui-place.json'"
check "/dev/turns needs the token" test "$(curl -s -m3 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/dev/turns")" = 401
none_running() { [ "$(token_turns)" = '{"running":0}' ]; }
wait_for 40 none_running; check "/dev/turns goes back to none when the turn ends" none_running
# a place is the row at the top and the px into it: `y` counts only the rows the list has measured, so it is not the same after a relaunch
place_of() { grep "\"name\":\"$1\"" "$SW/$2" 2>/dev/null | tail -1 | python3 -I -c 'import json,sys; t=sys.stdin.read(); v=json.loads(t.split(" ",1)[1]) if t else None; print(v["top"]+"+"+str(round(v["px"])) if v else "none")'; }
Y0=$(place_of left client.log); echo "left the thread scrolled to $Y0"
scrolled() { case "$Y0" in ''|none|'(end)'*) return 1;; esac; }
check "the thread was scrolled up from its end" scrolled
L0=$(wc -l < "$SW/home/events.jsonl")

# 2. the watcher follows; a build with the same hub swaps only the window, on SIGTERM, and the place comes back
SWITCH_CLIENT_ARGS="--script $T/back.json" "$HERE/scripts/gpui/dev-reload.sh" < /dev/null > "$T/watch.log" 2>&1 & WP=$!
H0=$(pid hub); C0=$(pid client); c1=$(cut_build "$B")
wait_for 40 shown "$c1"; check "the window build was taken up" shown "$c1"
check "the hub was left running" test "$(pid hub)" = "$H0"
check "the window is a new process" test "$(pid client)" != "$C0"
check "the new window opened on the same thread" log_has "\"thread\":\"$T1\".*\"event\":\"place\"" client.log
wait_for 10 log_has '"event":"usable"' client.log
check "it shows that thread's entries" sh -c "grep '\"event\":\"usable\"' '$SW/client.log' | grep -qv '\"entries\":0'"
wait_for 10 log_has '"name":"back"' client.log; Y1=$(place_of back client.log); echo "came back at $Y1"
check "the scroll place came back" sh -c "[ -n '$Y0' ] && [ '$Y1' = '$Y0' ]"
sleep 1; DRAFTED=$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get(sys.argv[2],""))' "$SW/home/gpui-drafts.json" "$T1")
check "the draft is still kept" test "$DRAFTED" = "a draft kept across builds"
check "the old window saved and quit by itself (its log has no panic)" sh -c "! grep -qi panic '$SW/client.log.prev'"
DISPLAY=$D import -window root "$T/after-client-swap.png" 2>/dev/null

# 3. a new hub: hub and window both move, on the same copy
v2=$(variant hub2); H1=$(pid hub); c2=$(cut_build "$v2")
wait_for 60 shown "$c2"; check "the hub build was taken up" shown "$c2"
check "the hub is a new process" test "$(pid hub)" != "$H1"
check "the copy was kept, not made again" test "$(wc -l < "$SW/home/events.jsonl")" -ge "$L0"
wait_for 10 log_has '"name":"back"' client.log; Y2=$(place_of back client.log); check "the scroll place survived a hub restart too" sh -c "[ -n '$Y0' ] && [ '$Y2' = '$Y0' ]"
check "the window came back on the same thread" log_has "\"thread\":\"$T1\".*\"event\":\"place\"" client.log

# 4. a running turn holds a new hub back
kill "$WP" 2>/dev/null; wait "$WP" 2>/dev/null; WP=
echo '[{"wait":500},{"type":"hold:45000 long turn"},{"send":true},{"wait":1000}]' > "$T/send.json"
SWITCH_CLIENT_ARGS="--script $T/send.json" env HUB_BIN="$v2/backplane-hub" CLIENT_BIN="$B/backplane-gpui" "$HERE/scripts/gpui/switch.sh" reload client > /dev/null
busy() { case "$(token_turns)" in '{"running":0}'|'') return 1;; *) return 0;; esac; }
wait_for 20 busy; check "the turn is running" busy
"$HERE/scripts/gpui/dev-reload.sh" < /dev/null >> "$T/watch.log" 2>&1 & WP=$!
v3=$(variant hub3); H2=$(pid hub); c3=$(cut_build "$v3")
sleep 6
check "the hub was not restarted under a running turn" test "$(pid hub)" = "$H2"
check "the watcher said why" grep -q 'turn is running' "$T/watch.log"
idle() { ! busy; }
wait_for 70 idle; wait_for 60 shown "$c3"; check "the build was taken up once the turn ended" shown "$c3"
check "the hub moved then" test "$(pid hub)" != "$H2"
check "the turn ended by itself, not cut" sh -c "! grep -q '\"state\":\"interrupted\"' '$SW/home/events.jsonl'"

# 5. a build that does not come up is marked bad and the one before is put back
v4=$T/bad; mkdir -p "$v4"; cp /bin/false "$v4/backplane-hub"; cp "$B/backplane-gpui" "$v4/"; cp -r "$B/assets" "$v4/"
c4=$(cut_build "$v4")
bad() { [ "$(cat "$DEV/.bad" 2>/dev/null)" = "$c4" ]; }
wait_for 30 bad; check "the broken build was marked bad" bad
up() { curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello"; }
wait_for 60 up; check "the hub before it is back" up
wait_for 30 log_has '"event":"usable"' client.log; check "the window is back" log_has '"event":"usable"' client.log
check "still on the same thread" log_has "\"thread\":\"$T1\".*\"event\":\"place\"" client.log

# 6. a thread gone since: the window opens on the list, it does not stick on an error
kill "$WP" 2>/dev/null; wait "$WP" 2>/dev/null; WP=
python3 -I -c 'import json,sys; p=sys.argv[1]; v=json.load(open(p)); v["target"]["id"]="t-gone"; v["session"]["sel"]="t-gone"; json.dump(v,open(p,"w"))' "$SW/home/gpui-place.json"
# the running window would write its own place back on quit: stop it first, then edit, then start
env HUB_BIN="$(cat "$SW/hub.bin")" CLIENT_BIN="$B/backplane-gpui" "$HERE/scripts/gpui/switch.sh" reload client > /dev/null
wait_for 20 log_has '"event":"usable"' client.log; check "a gone thread still opens" log_has '"event":"usable"' client.log
grep -E 'place|Refused|refused|not_usable' "$SW/client.log" | cut -c1-200 | head -6
DISPLAY=$D import -window root "$T/gone.png" 2>/dev/null

echo; [ "$fails" = 0 ] && echo "all checks passed" || echo "$fails checks FAILED"
[ "$fails" = 0 ]
