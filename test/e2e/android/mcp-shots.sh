#!/bin/sh
# Settings > MCP servers on the Android app, against a temp hub seeded as scripts/desktop-mcp-shot.sh seeds the desktop's (its python is run from there:
# servers.json, state.json, a fake HOME with ~/.codex/config.toml and ~/.claude.json, stand-in agents, the file vault, no tailscale, loopback).
# Shots: the list, an opened card, the error card, Add with a paste, Gallery, Import; THEMES="dark light" (default both).
# usage: test/e2e/android/mcp-shots.sh HUB_BIN APK OUTDIR    (EMU_PORT: the emulator scripts/android-emu.sh runs; PORT: the hub's, default 3872)
set -eu
HUBBIN=$(realpath "$1"); APK=$(realpath "$2"); OUT=$(mkdir -p "$3" && realpath "$3")
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"
E="$ROOT/scripts/android-emu.sh"; UI="python3 $ROOT/scripts/android-ui.py"; A="$E adb shell"; PKG=dev.backplane.mobile.gpui
PORT=${PORT:-3872}; THEMES=${THEMES:-dark light}
R=$(mktemp -d "$BP_TMP/mcpdroid.XXXXXX"); mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fh/.codex" "$R/fh/.cache"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
# the desktop shot's seeding, as it is there (its client script left out), and its paste kept for the Add shot
SEED=$(sed -n "/^python3 - \"\$R\" <<'PY'$/,/^PY$/p" "$ROOT/scripts/desktop-mcp-shot.sh" | sed '1d;$d' | sed '/^st=\[{"select"/,/^json.dump(st,open(R+"\/script.json","w"))$/d')
echo "$SEED" | grep -q 'servers.json' && echo "$SEED" | grep -q '^PASTE=' || { echo "mcp-shots: the desktop script's seeding was not found"; exit 2; }
printf '%s\nopen(R+"/paste.txt","w").write(PASTE)\n' "$SEED" | python3 - "$R"

HUBPID=""
cleanup() { [ -n "$HUBPID" ] && kill "$HUBPID" 2>/dev/null || true; sleep 1; cp "$R/hub.log" "$OUT/mcp-android.hub.log" 2>/dev/null || true; rm -rf "$R"; }
trap cleanup EXIT INT TERM
( export BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 BACKPLANE_VAULT_KEYRING=file HOME=$R/fh XDG_CACHE_HOME=$R/fh/.cache XDG_CONFIG_HOME=$R/fh/.config XDG_DATA_HOME=$R/fh/.local LC_ALL=C
  unset DISPLAY WAYLAND_DISPLAY; cd "$R/proj"; exec nice -n 10 "$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground ) > "$R/hub.log" 2>&1 &
HUBPID=$!
for k in $(seq 1 60); do curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" && break; sleep 0.5; done
[ -s "$R/home/token" ] || { echo "mcp-shots: the hub did not start"; exit 2; }

shot() { $E adb exec-out screencap -p > "$OUT/$1.png"; echo "shot $OUT/$1.png"; }
has() { $UI has "$@" 2>/dev/null; }
want() { for i in $(seq 1 15); do has "$@" && return 0; sleep 1; done; echo "FAIL: '$1' not shown"; shot "fail-$(echo "$1" | tr -c 'a-zA-Z0-9' _)"; exit 1; }
tapt() { want "$@"; p=$($UI find "$@") ; $A input tap $p; sleep 1.5; }
# a card further down: scroll until its title is on screen
seek() { for i in 1 2 3 4 5 6; do has "$@" && { p=$($UI find "$@"); $A input tap $p; sleep 1.5; return 0; }; down 1100; done; echo "FAIL: '$1' not found"; exit 1; }
# a header button by its tag position: the hub's labels are also on cards, so the head's are found by bounds (the first match on screen is the head's)
top() { $A input swipe 540 700 540 2200 120; $A input swipe 540 700 540 2200 120; sleep 0.8; }
down() { $A input swipe 540 1800 540 ${1:-900} 400; sleep 1; }

# the app, fresh, paired with the temp hub (through the emulator's 10.0.2.2), its clock fixed
$E adb uninstall $PKG >/dev/null 2>&1 || true; $E adb install "$APK" | tail -1
$A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1 || true
$A settings put global sysui_demo_allowed 1; $A am broadcast -a com.android.systemui.demo -e command enter >/dev/null; $A am broadcast -a com.android.systemui.demo -e command clock -e hhmm 1200 >/dev/null
$A am broadcast -a com.android.systemui.demo -e command notifications -e visible false >/dev/null
$A input keyevent KEYCODE_HOME; sleep 1   # no keyboard or app left up from an earlier run
LINK="http://10.0.2.2:$PORT/#token=$(cat "$R/home/token")"; ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "$LINK")
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 5
# a link that pairs asks first (its Pair is the last "Pair" on screen); then the tour's Skip; until the hub's projects show
for i in $(seq 1 30); do
  if has "Pair with this hub?"; then set -- $($UI bounds "Pair" exact $(( $($UI count "Pair" exact) - 1 ))); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 3; continue; fi
  if has Skip exact; then $A input tap $($UI find Skip exact); sleep 2; continue; fi
  has "Settings" exact && has "esp32-motor" && break; sleep 1
done
want "esp32-motor"

for TH in $THEMES; do
  $A cmd uimode night $([ "$TH" = dark ] && echo yes || echo no) >/dev/null; sleep 3
  # back to the app's home: Back until Settings shows, the app started again if Back left it
  for i in 1 2 3 4 5; do has "Settings" exact && break
    if $A dumpsys window | grep -q "mCurrentFocus.*$PKG"; then $A input keyevent KEYCODE_BACK; else $A am start -n $PKG/dev.backplane.mobile.DataActivity >/dev/null; fi; sleep 1.5; done
  tapt "Settings" exact; tapt "MCP servers" exact; want "Microchip MPLAB Docs"; sleep 2
  shot "mcp-$TH-list"; down 700; shot "mcp-$TH-list-2"; top
  tapt "Microchip MPLAB Docs" exact; want "search_documentation"; sleep 1; shot "mcp-$TH-open"; down 1000; shot "mcp-$TH-open-2"; top
  tapt "Microchip MPLAB Docs" exact; sleep 1; seek "Bench PSU" exact; top; want "it exited with code 1 before it answered"; sleep 1; shot "mcp-$TH-error"; down 1000; shot "mcp-$TH-error-2"; top
  # Add: the head's first button; the paste typed line by line (input text has no new line: Enter between)
  set -- $($UI bounds "Add" exact 0); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 1.5; want "Add a server"
  p=$($UI edit 0); $A input tap $p; sleep 1
  python3 - "$R/paste.txt" <<'PY' | while IFS= read -r l; do if [ "$l" = "@@ENTER" ]; then $A input keyevent KEYCODE_ENTER </dev/null; else $A "input text '$l'" </dev/null; sleep 0.6; fi; done
import sys
t = open(sys.argv[1]).read().split("\n")
for i, l in enumerate(t):
    if l: print(l.replace("%", "%25").replace(" ", "%s"))
    if i < len(t) - 1: print("@@ENTER")
PY
  sleep 0.5; $A input keyevent KEYCODE_BACK; sleep 2; want "Found 2 servers"
  shot "mcp-$TH-add"; down 600; shot "mcp-$TH-add-2"; top
  set -- $($UI bounds "Gallery" exact 0); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 2; want "Checked to exist"
  shot "mcp-$TH-gallery"; down 600; shot "mcp-$TH-gallery-2"; top
  set -- $($UI bounds "Import" exact 0); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 3; want "Import from your tools"; want "context7"; sleep 1
  shot "mcp-$TH-import"; down 700; shot "mcp-$TH-import-2"; top
  $A input keyevent KEYCODE_BACK; sleep 1; $A input keyevent KEYCODE_BACK; sleep 1
done
$A am broadcast -a com.android.systemui.demo -e command exit >/dev/null
echo "shots in $OUT"
