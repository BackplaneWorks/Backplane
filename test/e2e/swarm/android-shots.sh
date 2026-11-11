#!/bin/bash
# Swarm screens on Android against a throwaway hub with stand-in agents: the project's swarm rows, the swarm screen, New swarm, a synthesis.
#   android-shots.sh HUB_BIN APK OUT       env: EMU_PORT (5612: this lane's own emulator), PORT (3887), HOST (this machine's tailnet address)
# The hub: a temp home under $BP_TMP, --no-tailscale, BACKPLANE_AGENTS = stand-ins only (claude is test/tools/claude_standin.ts; the rest exit).
# Only the hub this script started (its PID) is stopped at the end; the emulator is this lane's own unit (scripts/android-emu.sh).
set -u
ROOT=$(cd "$(dirname "$0")/../../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
HUB=$(realpath "$1"); APK=$(realpath "$2"); OUT=$3; mkdir -p "$OUT"; OUT=$(realpath "$OUT")
BUN=$(command -v bun || echo "$HOME/.bun/bin/bun"); [ -x "$BUN" ] || { echo "no bun"; exit 2; }
export EMU_PORT=${EMU_PORT:-5612}; PORT=${PORT:-3887}; HOST=${HOST:-$(tailscale ip -4 | head -1)}; PKG=dev.backplane.mobile.gpui
E() { sh "$ROOT/scripts/android-emu.sh" "$@"; }; U() { python3 "$ROOT/scripts/android-ui.py" "$@"; }
shot() { E adb exec-out screencap -p > "$OUT/$1.png"; echo "shot $OUT/$1.png"; }
tap() { local xy; for i in $(seq 1 20); do xy=$(U find "$@" 2>/dev/null) && { E adb shell input tap $xy; return 0; }; sleep 1; done; echo "not found: $*"; return 1; }
R=$BP_TMP/swarm-android/rig; rm -rf "$R"; mkdir -p "$R/home" "$R/bin" "$R/proj" "$R/fakehome" "$R/tmp"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$BUN" "$ROOT/test/tools/claude_standin.ts" > "$R/bin/claude"
for n in codex grok muse agy antigravity opencode pi omp; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; done; chmod +x "$R/bin/"*
python3 - "$R" <<'PY'
import json, sys, time
R = sys.argv[1]; now = int(time.time())
ev = [{"$": "ProjectCreated", "id": "p1", "title": "Front panel", "root": R + "/proj", "at": now - 900},
      {"$": "ThreadCreated", "id": "t-a", "project": "p1", "title": "Panel LEDs: brightness curve", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now - 800},
      {"$": "ThreadCreated", "id": "t-b", "project": "p1", "title": "Order the enclosure", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now - 700},
      {"$": "SettingSet", "key": "restart.continue", "value": "off", "at": now - 900}]
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
PY
(cd "$R/proj" && git init -q . && echo "# panel" > README.md && git add . && git -c user.name=i2cjak -c user.email=build@amemb.com commit -qm init)
env -i HOME="$R/fakehome" PATH="$R/bin:/usr/bin:/bin" TMPDIR="$R/tmp" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream BP_STREAM_N=2 BP_STREAM_MS=30 \
  BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 "$HUB" --home "$R/home" --port "$PORT" --host "$HOST" --no-tailscale --foreground --headless > "$R/hub.log" 2>&1 & HP=$!
trap 'kill $HP 2>/dev/null' EXIT
n=0; until curl -sf -m 2 "http://$HOST:$PORT/hello" >/dev/null; do n=$((n+1)); [ $n -gt 90 ] && { echo "the hub did not start"; tail "$R/hub.log"; exit 1; }; sleep 1; done
tr '\0' '\n' < "/proc/$HP/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub: stopping"; exit 3; }
"$BUN" "$ROOT/test/e2e/swarm/seed.ts" "http://$HOST:$PORT" "$R/home/token" board || { tail -30 "$R/hub.log"; exit 4; }
# HOLD=1: the hub stays up with its swarms (to drive the phone by hand) until $BP_TMP/swarm-android/rig/quit exists
[ -n "${HOLD:-}" ] && { echo "holding: http://$HOST:$PORT token $R/home/token"; until [ -e "$R/quit" ]; do sleep 2; done; exit 0; }
# SEED_ONLY=1: the hub and its swarms alone (checks the seed), then stop
[ -n "${SEED_ONLY:-}" ] && { "$BUN" "$ROOT/test/e2e/swarm/seed.ts" "http://$HOST:$PORT" "$R/home/token" synthesis; curl -s "http://$HOST:$PORT/hello" >/dev/null; grep -c '"RoomPosted"' "$R/home/events.jsonl"; exit 0; }

[ -d "$ROOT/build/gpui/android/avd" ] || E create >/dev/null
E start || exit 5
E adb install -r "$APK" | tail -1; E adb shell pm clear $PKG >/dev/null; E adb shell pm grant $PKG android.permission.POST_NOTIFICATIONS
E adb shell cmd uimode night yes >/dev/null
L=$(curl -s -m8 -X POST -H "Authorization: Bearer $(cat "$R/home/token")" "http://$HOST:$PORT/auth/invitations" | python3 -c 'import json,sys,urllib.parse;print(urllib.parse.quote(json.load(sys.stdin)["link"],safe=""))')
E adb shell am start -W -a android.intent.action.VIEW -d "'bpgpui://pair?url=$L'" $PKG >/dev/null; sleep 4
for i in $(seq 1 30); do U has "Pair with this hub?" exact >/dev/null 2>&1 && break; sleep 1; done; sleep 1
# the question sits over the pairing screen, which has a Pair of its own: the last one is the question's
set -- $(U bounds Pair exact 1); E adb shell input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); for i in $(seq 1 30); do U has "FRONT PANEL" exact >/dev/null 2>&1 && break; sleep 1; done; sleep 3
U has Dismiss exact >/dev/null 2>&1 && tap Dismiss exact; sleep 2   # the GitHub star suggestion
U has "Encoder debounce" >/dev/null 2>&1 || { tap "FRONT PANEL" exact; sleep 2; }
shot 01-project-rows
tap "Encoder debounce" exact; sleep 4; shot 02-swarm
E adb shell input keyevent 4; sleep 2
xy=$(U find "FRONT PANEL" exact) && E adb shell input swipe $xy $xy 900; sleep 2
tap "New swarm" exact; sleep 3
xy=$(U edit 0) && { E adb shell input tap $xy; E adb shell input text "Power%sbudget"; }
xy=$(U edit 1) && { E adb shell input tap $xy; E adb shell input text "Fit%sthe%spanel%sunder%s500%smA"; }
E adb shell input keyevent 111; sleep 1; shot 03-new-swarm
E adb shell input keyevent 4; sleep 2; U has "Footprint check" >/dev/null 2>&1 || E adb shell input keyevent 4; sleep 1
tap "Footprint check" exact; sleep 3
"$BUN" "$ROOT/test/e2e/swarm/seed.ts" "http://$HOST:$PORT" "$R/home/token" synthesis; sleep 4; shot 04-synthesis
grep -c '"RoomPosted"' "$R/home/events.jsonl" | sed 's/^/room posts in the log: /'
echo done
