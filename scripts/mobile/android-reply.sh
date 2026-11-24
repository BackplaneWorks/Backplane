#!/bin/bash
# Reply on the Android app (feature `reply`, docs/PHONE-DATA.md), through real taps on this desktop's headless emulator (scripts/android-emu.sh, never on
# the screen), against the copy rows' private hub (test/e2e/ios/copy-hub.mjs: thread t5 holds the person's cp0 and the agent's cp1) on a temp home.
#   scripts/mobile/android-reply.sh HUB_BIN APK [OUT]    EMU_PORT (default 5590: this lane's own emulator)
# Each step reads the UI tree (uiautomator); what was sent is checked in the hub's own log: the desktop's <in_reply_to msg=cp1 chars=N> block, and the
# stand-in's `zqecho` answer, which repeats what the agent was given. Stops what it started, by pid; the emulator stays up for the next run (stop it with
# `EMU_PORT=5590 scripts/android-emu.sh stop`).
set -u
HUB=$(realpath "${1:?hub binary}"); APK=$(realpath "${2:?debug apk}"); ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
OUT=${3:-$BP_TMP/android-reply}; rm -rf "$OUT"; mkdir -p "$OUT/shots"
export EMU_PORT=${EMU_PORT:-5590}; E=$ROOT/scripts/android-emu.sh; UI="python3 $ROOT/scripts/android-ui.py"; A="$E adb shell"; PKG=dev.backplane.mobile.gpui
PORT=${PORT:-$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));p=s.getsockname()[1];s.close();print(p)')}
. "$ROOT/test/e2e/lib/own-group.sh"; pids=(); fails=0
stop() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; own_stop; $E adb reverse --remove tcp:$PORT >/dev/null 2>&1; }
trap stop EXIT; trap "exit 143" INT TERM HUP
shot() { $E adb exec-out screencap -p > "$OUT/shots/$1.png"; }
xy() { $UI find "$@" 2>/dev/null; }
has() { $UI has "$@" 2>/dev/null; }
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; fails=$((fails+1)); shot "fail-$fails"; $UI dump > /dev/null; }
want() { local what=$1; shift; for i in $(seq 10); do has "$@" && { ok "$what"; return 0; }; sleep 1; done; bad "$what ('$1' not shown)"; return 1; }
gone() { local what=$1; shift; for i in $(seq 6); do has "$@" || { ok "$what"; return 0; }; sleep 1; done; bad "$what ('$1' still shown)"; return 1; }
tapt() { local p; p=$(xy "$@") || { bad "no '$1' to tap"; return 1; }; $A input tap $p; sleep 1.2; }
longt() { local p; p=$(xy "$@") || { bad "no '$1' to long-press"; return 1; }; set -- $p; $A input swipe $1 $2 $1 $2 900; sleep 1.2; }
swiper() { local p; p=$(xy "$@") || { bad "no '$1' to swipe"; return 1; }; set -- $p; $A input swipe $(( $1 - 120 )) $2 $(( $1 + 160 )) $2 350; sleep 1.2; }
# the last node with this exact text (a menu's item is drawn after the list); false when there is none
tapl() { local n; n=$($UI count "$1" exact 2>/dev/null || echo 0); [ "${n:-0}" -gt 0 ] || { bad "no '$1' to tap"; return 1; }; set -- $($UI bounds "$1" exact $(( n - 1 ))); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 1.2; }
# a fresh AVD's Bluetooth crashes once over whatever is on screen (scripts/android-emu.sh): close its dialog
nobt() { for i in 1 2 3; do has "keeps stopping" || return 0; tapt "Close app" exact; sleep 1; done; }
logged() { for i in $(seq ${2:-20}); do grep -Eq "$1" "$OUT/hub/home/events.jsonl" && return 0; sleep 1; done; return 1; }

# the hub, and the emulator (its own port, headless)
own bun "$ROOT/test/e2e/ios/copy-hub.mjs" "$HUB" "$OUT/hub" "$PORT" > "$OUT/hub.out" 2>&1; pids+=($!)
for _ in $(seq 60); do [ -f "$OUT/hub/state.json" ] && break; sleep 1; done
[ -f "$OUT/hub/state.json" ] || { echo "the hub did not start"; tail -5 "$OUT/hub.out"; exit 1; }
[ -d "$ROOT/build/gpui/android/avd" ] || sh "$E" create >/dev/null
sh "$E" start || exit 1
$E adb install -r "$APK" >/dev/null || { echo "install failed"; exit 1; }
$E adb reverse tcp:$PORT tcp:$PORT >/dev/null
TOK=$(python3 -c "import json;print(json.load(open('$OUT/hub/state.json'))['token'])")
ENC=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1],safe=''))" "http://127.0.0.1:$PORT/#token=$TOK")
$A pm clear $PKG >/dev/null; $A pm grant $PKG android.permission.POST_NOTIFICATIONS >/dev/null 2>&1
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$ENC'" >/dev/null; sleep 6; nobt
if has "Pair with this hub?"; then set -- $($UI bounds "Pair" exact $(( $($UI count "Pair" exact) - 1 ))); $A input tap $(( ($1 + $3) / 2 )) $(( ($2 + $4) / 2 )); sleep 4; fi
nobt; has "Skip" exact && tapt "Skip" exact; nobt

# a paired app opens on the uberbot's chat (Bub), its home: the thread by its link
$A "am start -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://thread/t5'" >/dev/null; sleep 3; nobt
want "the thread is open" "Needs a package"
want "the agent's reply is shown" "Then tell me"; shot 01-thread

# long-press the reply's footer: Reply and Copy; Reply puts the strip over the box
# (the footer is the time, then Reply: the press goes on the time, just left of Reply)
n=$($UI count "Reply" exact 2>/dev/null || echo 0); if [ "${n:-0}" -gt 0 ]; then set -- $($UI bounds "Reply" exact $(( n - 1 ))); $A input swipe $(( $1 - 50 )) $(( ($2 + $4) / 2 )) $(( $1 - 50 )) $(( ($2 + $4) / 2 )) 900; sleep 1.2; else bad "no Reply under the agent's message"; fi
# (the menu is a popup window of its own: uiautomator reads only it, so its Reply and Copy are what is on screen)
want "a long-press offers Copy" "Copy" exact; want "and Reply" "Reply" exact; shot 02-menu
tapl "Reply"
want "the strip names the sender" "Agent" exact; want "and the first line" "Replying to: One thing needs you"; shot 03-strip
tapt "Cancel reply"; gone "× drops the reply" "Replying to:"

# Reply under the message, then a send
tapl "Reply"
want "Reply under the message puts the strip back" "Replying to: One thing needs you"
p=$($UI edit 0) && $A input tap $p; sleep 1; $A input text "zqecho%smake%sit%sso"; sleep 1
tapt "Send" exact
if logged 'in_reply_to msg=cp1 chars=[0-9]+>\\nOne thing needs you' 20; then ok "the hub keeps the desktop's in_reply_to block"; else bad "no in_reply_to block in the hub's log"; fi
if logged '"role":"assistant".*echo: .*<in_reply_to msg=cp1 chars=[0-9]+>.*</in_reply_to>\\n\\nzqecho make it so' 40; then ok "the agent is given the block ahead of the words"; else bad "the agent's echo has no block"; fi
gone "the strip is gone after the send" "Replying to:"
want "the sent message shows what it answers" "In reply to: One thing needs you"; shot 04-sent

# a swipe right on the person's own message replies to it
for i in 1 2 3; do has "can you speed up the builds" && break; $A input swipe 540 700 540 1500 300; sleep 1; done
swiper "can you speed up the builds"
want "a swipe right is Reply, sender You" "You" exact && want "with its first line" "Replying to: zqshort can you speed up"; shot 05-swiped
tapt "Cancel reply"

# a tap on the quote goes to the original
$A input swipe 540 1500 540 600 300; sleep 1
tapt "In reply to: One thing needs you"; want "the original is on screen" "Then tell me"; shot 06-went

echo "android reply: $fails failed"; [ $fails = 0 ]
