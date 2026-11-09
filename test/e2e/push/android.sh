#!/bin/bash
# Phone alerts on a real Android phone (or an emulator) against a throwaway hub: test/e2e/push/android.sh SERIAL HUB_BIN APK [OUT]
# The hub is test/e2e/push/ctl.mjs (stand-in agents making the real MCP calls); the phone reaches it through `adb reverse`, so its only hub is this one.
# Installs the test app (dev.backplane.mobile.gpui) with fresh data and pairs it by link; never touches another package. What arrived is read from the phone's own
# notification list (dumpsys notification), never from the hub alone; a step that must see nothing waits QUIET seconds and checks the hub's count too.
set -u
SERIAL=${1:?adb serial}; HUB=$(realpath "${2:?hub binary}"); APK=$(realpath "${3:?debug apk}"); ROOT=$(cd "$(dirname "$0")/../../.." && pwd); OUT=${4:-$ROOT/build/push-e2e/android}
PKG=${PKG:-dev.backplane.mobile.gpui}; SCHEME=${SCHEME:-bpgpui}; [ "$PKG" = dev.backplane.mobile ] && { echo "never the person's own app (dev.backplane.mobile)"; exit 1; }; PORT=${PORT:-3870}; CTL=http://127.0.0.1:$((PORT+1)); QUIET=${QUIET:-10}
mkdir -p "$HOME/.cache/respectful-push" "$OUT"; T=$(mktemp -d "$HOME/.cache/respectful-push/and.XXXXXX"); rm -f "$OUT"/*.png
A() { adb -s "$SERIAL" "$@"; }
. "$ROOT/test/e2e/lib/own-group.sh"; pids=(); stop() { curl -s -m3 "$CTL/stop" >/dev/null 2>&1; sleep 1; for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; own_stop; A reverse --remove tcp:$PORT >/dev/null 2>&1; rm -rf "$T"; }
trap stop EXIT; trap "exit 143" INT TERM HUP
cd "$ROOT/test/e2e/push"
MODE=sink PERF_REPO=$ROOT own bun ctl.mjs "$T" "$HUB" "$PORT" > "$OUT/ctl.log" 2>&1; pids+=($!)
for _ in $(seq 60); do grep -q '^ready' "$OUT/ctl.log" && break; sleep 1; done
grep -q '^ready' "$OUT/ctl.log" || { echo "the hub did not start"; tail -5 "$OUT/ctl.log"; exit 1; }
TOK=$(cat "$T/home/token")
A reverse tcp:$PORT tcp:$PORT >/dev/null || { echo "adb reverse failed"; exit 1; }

pass=0; fail=0; R="$OUT/result.txt"; : > "$R"
step() { if [ "$2" = ok ]; then pass=$((pass+1)); echo "PASS $1${3:+ ($3)}" | tee -a "$R"; else fail=$((fail+1)); echo "FAIL $1${3:+ ($3)}" | tee -a "$R"; fi; }
shot() { A exec-out screencap -p > "$OUT/$1.png" 2>/dev/null; }
ctl() { curl -s -m45 "$CTL$1"; }
num() { ctl /state | python3 -c "import json,sys;print(json.load(sys.stdin).get('$1',-1))"; }
# this app's posted notifications, one line each: text|tag
notes() { A shell dumpsys notification --noredact 2>/dev/null | python3 -c "
import sys,re
cur=None;out=[]
for l in sys.stdin:
    m=re.search(r'NotificationRecord\(0x[0-9a-f]+: pkg=(\S+)',l)
    if m: cur=m.group(1)==sys.argv[1]; continue
    if cur:
        t=re.search(r'android\.text=String \((.*)\)\s*$',l)
        if t: out.append(t.group(1))
print('\n'.join(out))" "$PKG"; }
has() { notes | grep -qF "$1"; }
arrives() { local name=$1 text=$2; shift 2; "$@" >/dev/null; local ok=no; for _ in $(seq 45); do has "$text" && { ok=ok; break; }; sleep 1; done; step "$name" $ok; shot "${name// /-}"; }
silent() { local name=$1 text=$2; shift 2; local b; b=$(num sent); "$@" > "$T/said" 2>&1; sleep "$QUIET"; local a; a=$(num sent); local seen=no; has "$text" && seen=yes
  [ "$seen" = no ] && [ "$a" = "$b" ] && step "$name" ok "PushSent $b -> $a" || step "$name" no "banner $seen, PushSent $b -> $a: $(head -c 160 "$T/said")"; shot "${name// /-}"; }
home() { A shell input keyevent KEYCODE_HOME; sleep 1; }

A install -r -g "$APK" > "$OUT/install.log" 2>&1 || { echo "install failed"; cat "$OUT/install.log"; exit 1; }
A shell pm clear $PKG >/dev/null; A shell pm grant $PKG android.permission.POST_NOTIFICATIONS 2>/dev/null
A shell cmd notification allow_listener >/dev/null 2>&1
LINK=$(python3 -c "import urllib.parse,sys;print('$SCHEME://pair?url='+urllib.parse.quote('http://127.0.0.1:$PORT/#token=$TOK',safe=''))")
A shell am start -W -a android.intent.action.VIEW -d "'$LINK'" $PKG > /dev/null
# the question before an outside link pairs: tap its Pair once it is on screen
ui() { A shell uiautomator dump /sdcard/bp-ui.xml >/dev/null 2>&1; A shell cat /sdcard/bp-ui.xml 2>/dev/null; A shell rm -f /sdcard/bp-ui.xml; }
for _ in $(seq 20); do x=$(ui); if grep -q 'Pair with this hub' <<<"$x"; then
  xy=$(python3 -c "import sys,re;s=sys.stdin.read();m=list(re.finditer(r'text=\"Pair\"[^>]*bounds=\"\[(\d+),(\d+)\]\[(\d+),(\d+)\]\"',s));b=m[-1].groups();print((int(b[0])+int(b[2]))//2,(int(b[1])+int(b[3]))//2)" <<<"$x"); A shell input tap $xy; break; fi; sleep 1; done
# paired: the app's own screen lists the test hub's worker thread
ok=no; for _ in $(seq 30); do ui | grep -q 'Worker thread' && { ok=ok; break; }; sleep 1; done
shot paired; step "the phone pairs with the test hub" $ok "its list shows the hub's Worker thread"
home

first="Approve the deploy to staging?"
arrives "the uberbot's notify_person arrives" "$first" ctl "/uber?text=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$first'))")"
src=$(ctl /state | python3 -c "import json,sys;s=json.load(sys.stdin);l=s.get('lastSent') or {};print('ok' if l.get('thread')==s.get('bubThread') and l.get('title')=='Bub' else 'no', l.get('title'), l.get('thread'), s.get('bubThread'))")
step "the alert came from the uberbot" "${src%% *}" "${src#* }"
silent "another bot cannot push" "mochi wants a push" ctl "/other?text=mochi%20wants%20a%20push"
silent "a worker thread cannot push" "worker wants a push" ctl "/worker?text=worker%20wants%20a%20push"
silent "a finished turn does not push" "Finished" ctl /workerdone
ask=$(ctl /ask | python3 -c "import json,sys;print(json.load(sys.stdin).get('ask',''))"); [ -n "$ask" ] && step "a worker's approval is open" ok "$ask" || step "a worker's approval is open" no
silent "the approval alone does not push" "rm -rf" true
need="The worker needs your approval"
arrives "the uberbot's approval alert arrives" "$need" ctl "/uber?text=The%20worker%20needs%20your%20approval&ask=$ask"
ctl "/answer?ask=$ask" >/dev/null; ok=no; for _ in $(seq 20); do has "$need" || { ok=ok; break; }; sleep 1; done
step "answering it clears it from the phone" $ok; shot cleared
silent "a duplicate is suppressed" "$first" ctl "/uber?text=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$first'))")"
notes > "$OUT/notifications.txt"
grep -E 'push:|data' "$T/hub.log" | sed -E 's/[0-9a-f]{64}/<token>/g' | tail -30 > "$OUT/hub-push.log"
A shell am force-stop $PKG
echo "passed $pass failed $fail" | tee -a "$R"; [ "$fail" = 0 ]
