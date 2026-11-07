#!/bin/bash
# First run from a blank home, in a private window: Xvfb + i3, a temp HOME, stand-in agents, real clicks and keys. Nothing here touches the person's screen, home or sign-ins.
#   test/onboarding/run.sh BIN_DIR ASSETS_DIR OUT_DIR [XVFB]      BIN_DIR holds backplane-hub and backplane-gpui side by side
# DISP (:243) and PORT (a free one) pick the display and the hub's port. Screenshots land in OUT_DIR, one per step. Prints PASS/FAIL lines; exit 1 on any FAIL.
# Temp files stay under OUT_DIR/rig, on the same disk as OUT_DIR: /tmp is a shared tmpfs with a quota.
set -u
BIN=$1; ASSETS=$2; OUT=$3; XVFB=${4:-/tmp/xvfb-root/usr/bin/Xvfb}; D=$(cd "$(dirname "$0")" && pwd)
export DISPLAY=${DISP:-:243}
# a port nobody holds (other lanes run hubs and servers on this machine); PORT overrides
PORT=${PORT:-$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')}
R=$OUT/rig; rm -rf "$R"; mkdir -p "$R/home" "$R/run" "$R/agents" "$R/noagents" "$OUT/shots"; chmod 700 "$R/run"
fails=0; pids=(); client=
cleanup() { [ -n "$client" ] && kill "$client" 2>/dev/null; [ -f "$R/hub.pid" ] && kill "$(cat "$R/hub.pid")" 2>/dev/null; kill "${pids[@]}" 2>/dev/null; }
trap cleanup EXIT
ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
HOME_DIR=$R/home; HUB_HOME=$HOME_DIR/.backplane/hub
case "$(realpath -m "$HOME_DIR")" in "$HOME"/.backplane*|"$HOME") echo "refused: not a temp home"; exit 2;; esac

# stand-ins: each answers only what setup asks. They are the only agents this run can find.
cat > "$R/agents/claude" <<'EOF'
#!/bin/sh
case "$1 $2" in "--version "*) echo "2.1.300 (Claude Code)";; "auth status") echo '{"loggedIn":true,"email":"standin@example.com","authMethod":"claude.ai"}';; *) exit 0;; esac
EOF
cat > "$R/agents/codex" <<'EOF'
#!/bin/sh
case "$1 $2" in "--version "*) echo "codex-cli 0.9.1";; "login status") echo "Not logged in" >&2; exit 1;; *) exit 0;; esac
EOF
cat > "$R/agents/freecadcmd" <<'EOF'
#!/bin/sh
echo "FreeCAD 1.0.2 Revision: 39319 (Git)"
EOF
chmod +x "$R/agents/"*

$XVFB "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >"$R/xvfb.log" 2>&1 & pids+=($!); sleep 2
printf 'ipc-socket %s/i3.sock\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\n' "$R" >"$R/i3.conf"
env -u I3SOCK -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$R/run" i3 -c "$R/i3.conf" >"$R/i3.log" 2>&1 & pids+=($!); sleep 1

# AGENTS names the folder of stand-ins for this launch; TOOLS adds to PATH
launch() { # launch AGENTS [extra env...]
  local agents=$1; shift
  env -i HOME="$HOME_DIR" XDG_CONFIG_HOME="$HOME_DIR/.config" XDG_RUNTIME_DIR="$R/run" DISPLAY="$DISPLAY" PATH="$agents:/usr/bin:/bin" TMPDIR="$R" \
    BACKPLANE_AGENTS="$agents" BACKPLANE_NO_TAILSCALE=1 BACKPLANE_PORT="$PORT" BACKPLANE_NO_UPDATE=1 BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_PEERS= \
    BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1 LP_NUM_THREADS=2 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json "$@" "$BIN/backplane-gpui" >>"$R/win.log" 2>&1 &
  client=$!
}
X() { python3 "$D/../bub-send/xdo.py" "$@"; }
shot() { import -window root "$OUT/shots/$1.png"; }
log() { cat "$HUB_HOME/events.jsonl" 2>/dev/null; }
setting() { log | python3 -c "
import sys,json
v=''
for l in sys.stdin:
    try: e=json.loads(l)
    except Exception: continue
    if e.get('\$')=='SettingSet' and e.get('key')=='$1': v=e.get('value','')
print(v)"; }
# true once $1 holds; false after $2 half-seconds (an until loop answers its body's last status, so the 0 is said)
wait_for() { local n=0; until eval "$1"; do sleep 0.5; n=$((n+1)); [ $n -gt ${2:-60} ] && return 1; done; return 0; }
# the port the hub took, from its lock ("PID PORT flock"); PORT until the lock is there
hub_port() { local p; p=$(cut -d' ' -f2 "$HUB_HOME/hub.lock" 2>/dev/null); echo "${p:-$PORT}"; }
hub_up() { curl --noproxy "*" -sf -m 3 "http://127.0.0.1:$(hub_port)/hello" >/dev/null; }

# 1. a blank machine: no home, no choice saved, the chooser
[ ! -e "$HUB_HOME" ] && ok "the home does not exist before the first launch" || bad "a home was there before the first launch"
launch "$R/noagents" BACKPLANE_KICAD_CLI=/nonexistent/kicad-cli
sleep 6; shot 00-choose-home
hub_up && bad "a hub started before a home was chosen" || ok "no hub runs until a home is chosen"
[ ! -e "$HUB_HOME" ] && ok "the chooser made no folder" || bad "the chooser made a folder by itself"

# the window is 1400x900 centred by i3 at full screen; First centres a 620 px column. The Start button is found by its rectangle in the client's log.
pos() { python3 "$D/pos.py" "$R/win.log" "$1"; }
X click $(pos first-use); wait_for hub_up 120 && ok "Use this folder started a hub" || { bad "no hub answered"; date +%T.%N; cat "$HUB_HOME/hub.lock"; curl --noproxy "*" -sv -m 5 "http://127.0.0.1:$(hub_port)/hello" -o /dev/null 2>&1 | tail -5; }
[ -f "$HUB_HOME/token" ] && ok "the hub made its home at the default path" || bad "no token in $HUB_HOME"
wait_for "[ -f '$HOME_DIR/.config/backplane-gpui/launch.json' ]" 20 && ok "the choice was saved" || { bad "no launch.json"; date +%T.%N; grep launch_saved "$R/win.log"; ls -la "$HOME_DIR/.config/backplane-gpui"; }
head -c 12 "$HUB_HOME/hub.lock" >"$R/hub.lock.head"; cut -d' ' -f1 "$HUB_HOME/hub.lock" >"$R/hub.pid"
tr '\0' '\n' <"/proc/$(cat "$R/hub.pid")/environ" | grep -q "BACKPLANE_AGENTS=$R/noagents" && ok "the hub runs with the stand-in folder" || { bad "stand-in folder not set on the hub"; exit 1; }
sleep 5; shot 01-welcome
wait_for '[ -n "$(setting onboard.checks)" ]' 80 && ok "the checks ran on a blank machine" || bad "no checks"
setting onboard.checks >"$R/checks-blank.json"
python3 - "$R/checks-blank.json" <<'EOF' && ok "a blank machine finds no agents or KiCad" || bad "the blank machine's checks are wrong"
import json,sys
c=json.load(open(sys.argv[1]))
assert all(not v.get('path') for v in c['agents'].values()), c['agents']
assert not c['kicad'].get('runs'), c['kicad']
EOF

# 2. every step, with Enter, skipping what is not done
ids=(welcome install home usage claude codex others kicad freecad machine phone remote project thread viewers model3d mech browser threadtour bots space rooms routines palette settings uber done)
i=0
for id in "${ids[@]}"; do
  i=$((i+1)); n=$(printf %02d $((i+1)))
  sleep 1.5; shot "$n-$id"
  [ "$id" = done ] && break
  [ "$id" = kicad ] && { X key Escape; sleep 2; kill "$client"; wait "$client" 2>/dev/null; client=; sleep 1
    [ "$(setting onboard.step)" = kicad ] && ok "Finish later kept the step" || bad "step after Finish later: $(setting onboard.step)"
    hub_up && ok "the hub kept running when the window closed" || bad "the hub stopped with the window"
    launch "$R/agents" BACKPLANE_FREECAD="$R/agents/freecadcmd" BACKPLANE_KICAD_CLI=/usr/bin/kicad-cli
    sleep 8; shot "$n-kicad-resumed"
    [ "$(setting onboard.step)" = kicad ] && ok "a new launch came back to the same step" || bad "resumed at $(setting onboard.step)"
    # the launcher attaches to the running hub, whose PATH has no stand-ins: the rest of the run restarts it below
  }
  X key Return
done
echo "steps walked: $i of ${#ids[@]}"
[ "$(setting onboard.step)" = done ] && ok "reached the last step" || bad "stopped at $(setting onboard.step)"
s=$(setting onboard.skipped); case ",$s," in *,claude,*) ok "unfinished steps were recorded as skipped: $s";; *) bad "skipped list: $s";; esac
X key Return; sleep 3; shot 90-finished
[ "$(setting onboard.done)" = 1 ] && ok "Done finished setup" || bad "onboard.done is '$(setting onboard.done)'"

# 3. a later launch stays out of the way, and Settings runs it again
kill "$client"; wait "$client" 2>/dev/null; client=; launch "$R/agents"; sleep 8; shot 91-second-launch
echo "fails=$fails"; [ "$fails" = 0 ]
