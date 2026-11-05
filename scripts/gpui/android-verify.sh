#!/bin/sh
# One verification round: a fresh hub home with N threads (stand-in agent, environ checked), a fresh pair through the logging proxy,
# time to Connected, join size, thread open, a send round trip. usage: android-verify.sh N APK LABEL
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/gpui/android-emu.sh; U="python3 $ROOT/scripts/gpui/android-ui.py"; H=$HOME/bp-gpui-android-hub
N=$1; APK=$2; L=$3; R=$ROOT/build/gpui/results/android/verify; S=$ROOT/build/gpui/shots/android/verify; mkdir -p $R $S
HD=$H/home.v$N; OUT=$R/$L.txt; : > $OUT; say() { echo "$*" | tee -a $OUT; }
$ROOT/scripts/gpui/android-hub.sh stop >/dev/null 2>&1; [ -f $H/proxy.pid ] && kill $(cat $H/proxy.pid) 2>/dev/null; sleep 1
rm -rf $HD; mkdir -p $HD; HOMEDIR=$HD $ROOT/scripts/gpui/android-hub.sh start >/dev/null 2>&1
HP=$(pgrep -f "backplane-serve --home $HD" | head -1); ENVOK=$(tr '\0' '\n' < /proc/$HP/environ | grep -c "^BACKPLANE_AGENTS=$H/bin")
say "$L: threads=$N apk=$(basename $APK) hub_pid=$HP BACKPLANE_AGENTS_ok=$ENVOK commit=$(git -C $ROOT rev-parse --short HEAD) bridge_sha=$(sha256sum $ROOT/build/gpui/bridge/bridge.js | cut -c1-12)"
[ "$ENVOK" = 1 ] || { say "ABORT: stand-in not in hub environ"; exit 1; }
timeout 30 bun $ROOT/scripts/gpui/ios-seed.ts 100.64.84.41:3814 $HD $H/proj >/dev/null 2>&1
PID=$(grep ProjectCreated $HD/events.jsonl | grep -o '"id":"p[0-9-]*"' | cut -d'"' -f4)
[ "$N" -gt 1 ] && timeout 400 bun $ROOT/scripts/gpui/android-seed.ts 100.64.84.41:3814 $HD $PID $((N-1)) >/dev/null 2>&1
say "threads in log: $(grep -c ThreadCreated $HD/events.jsonl)"
PL=$R/$L.proxy.log; rm -f $PL*; (cd /tmp && nohup bun $ROOT/scripts/gpui/android-proxy.ts 100.64.84.41:3815 100.64.84.41:3814 $PL >/dev/null 2>&1 & echo $! > $H/proxy.pid); sleep 1
$E adb uninstall dev.backplane.mobile.gpui >/dev/null 2>&1; $E adb install $APK | tail -1 | tee -a $OUT; $E adb shell pm clear dev.backplane.mobile.gpui >/dev/null; $E adb shell pm grant dev.backplane.mobile.gpui android.permission.POST_NOTIFICATIONS; $E adb logcat -c
T0=$(date +%s.%N); PORT=3815 TOKENFILE=$HD/token $ROOT/scripts/gpui/android-pair.sh >/dev/null
CT=""; for i in $(seq 1 60); do if $U has "Connected" || $U has "Skip" exact; then CT=$(echo "$(date +%s.%N) - $T0" | bc); break; fi; done
say "pair -> Connected (uiautomator poll, ~1 s resolution): ${CT:-never} s"
sleep 2
if $U has "Skip" exact; then set -- $($U find Skip exact); $E adb shell input tap $1 $2; sleep 1; fi
ROWS=$($U count "Seed thread" ); say "rows visible on screen: $ROWS"
$E adb exec-out screencap -p > $S/$L-list.png
# join frames: greeting, then the join reply (and control parts when segmented)
python3 - "$PL" <<'P' | tee -a $OUT
import re,sys,glob
log=sys.argv[1]; lines=[l for l in open(log) if "s2c frame#" in l and "len=" in l]
conns={}
for l in lines:
    m=re.match(r"([\d.]+) conn#(\d+) s2c frame#(\d+) op=\d+ fin=\d+ len=(\d+)",l)
    if m: conns.setdefault(int(m[2]),[]).append((float(m[1]),int(m[3]),int(m[4])))
ws=[c for c,f in conns.items() if len(f)>=2]
c=max(ws); f=conns[c]
big=[x for x in f if x[2]>5]
sizes=[x[2] for x in f if x[2]>5]
print(f"wire: ws conn#{c}: frames={len(f)} (heartbeats <=5 B: {len(f)-len(sizes)}); big frames in order: {sizes}; greeting->join reply {f[1][0]-f[0][0]:.3f} s (join reply = {sizes[1] if len(sizes)>1 else '-'} B)")
P
if [ "$N" -gt 48 ] && [ "${NOSIDEBAR:-0}" != 1 ]; then
  for i in $(seq 1 14); do $E adb shell input swipe 540 1900 540 500 150; sleep 0.3; done; $E adb exec-out screencap -p > $S/$L-page1-bottom.png
  if xy=$($U find Next); then say "page controls: Next found at page 1 bottom"; $E adb shell input tap $xy; sleep 4; for i in $(seq 1 14); do $E adb shell input swipe 540 1900 540 500 150; sleep 0.3; done; $E adb exec-out screencap -p > $S/$L-page2-bottom.png; $U find Previous >/dev/null && say "page controls: Previous found on page 2"; else say "page controls: NO Next control found"; fi
  for i in $(seq 1 16); do $E adb shell input swipe 540 600 540 1900 150; sleep 0.2; done
fi
# open the first thread row, send, wait for the stand-in's reply
set -- $($U find "Seed thread" ); $E adb shell input tap 450 ${2:-746}; sleep 4
$E adb exec-out screencap -p > $S/$L-thread.png
set -- $($U find "Ask the agent" ); $E adb shell input tap $1 $2; sleep 1; $E adb shell input text "hello%sfrom%s$L"; sleep 1
set -- $($U find "Send" exact ); T1=$(date +%s.%N); $E adb shell input tap $1 $2
RT=""; for i in $(seq 1 40); do if $U has "${L}11"; then RT=$(echo "$(date +%s.%N) - $T1" | bc); break; fi; done
say "send -> stand-in reply visible: ${RT:-never} s"
$E adb exec-out screencap -p > $S/$L-reply.png
say "turns: TurnUsage events=$(grep -c TurnUsage $HD/events.jsonl) (a real model would log one with cost); SessionSet: $(grep -h SessionSet $HD/events.jsonl | cut -c1-110 | head -1); assistant msg ids: $(grep -h '"role":"assistant"' $HD/events.jsonl | grep -o '"msg":"[^"]*"' | head -1)"
say "webview nodes: $($U count android.webkit.WebView) ; user message shown: $($U has "hello from $L" && echo yes || echo no)"
$E adb logcat -d | grep -E "view failed" | cut -c1-120 | tee -a $OUT
