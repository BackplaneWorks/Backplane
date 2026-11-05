#!/bin/sh
# Fix 3 end to end: a thread whose view is too big for one reply (a ~100 KB message) arrives as view.control parts + reply (sidebar bounded, 6 threads).
# usage: android-segmented.sh APK
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); E=$ROOT/scripts/gpui/android-emu.sh; U="python3 $ROOT/scripts/gpui/android-ui.py"; H=$HOME/bp-gpui-android-hub
APK=$1; R=$ROOT/build/gpui/results/android/verify; S=$ROOT/build/gpui/shots/android/verify; HD=$H/home.seg; OUT=$R/segmented.txt; : > $OUT; say() { echo "$*" | tee -a $OUT; }
$ROOT/scripts/gpui/android-hub.sh stop >/dev/null 2>&1; [ -f $H/proxy.pid ] && kill $(cat $H/proxy.pid) 2>/dev/null; sleep 1
rm -rf $HD; mkdir -p $HD; HOMEDIR=$HD $ROOT/scripts/gpui/android-hub.sh start >/dev/null 2>&1
HP=$(pgrep -f "backplane-serve --home $HD" | head -1); ENVOK=$(tr '\0' '\n' < /proc/$HP/environ | grep -c "^BACKPLANE_AGENTS=$H/bin"); say "segmented: hub_pid=$HP BACKPLANE_AGENTS_ok=$ENVOK apk=$(basename $APK) commit=$(git -C $ROOT rev-parse --short HEAD)"; [ "$ENVOK" = 1 ] || exit 1
timeout 30 bun $ROOT/scripts/gpui/ios-seed.ts 100.64.84.41:3814 $HD $H/proj >/dev/null 2>&1
PID=$(grep ProjectCreated $HD/events.jsonl | grep -o '"id":"p[0-9-]*"' | cut -d'"' -f4); timeout 60 bun $ROOT/scripts/gpui/android-seed.ts 100.64.84.41:3814 $HD $PID 5 >/dev/null 2>&1
T1=$(grep ThreadCreated $HD/events.jsonl | sed -n 2p | grep -o '"id":"t[0-9-]*"' | cut -d'"' -f4)
mk() { python3 -c "import sys;print('\n'.join('$1 %04d lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor' % i for i in range(1200)) + '\nendmark')"; }
BIG=$(mk line); BIG2=$(mk more)
say "message bytes: $(printf '%s' "$BIG" | wc -c)"
timeout 100 bun $ROOT/scripts/gpui/android-seed.ts 100.64.84.41:3814 $HD x 0 turn $T1 "$BIG" $T1 "$BIG2" >/dev/null 2>&1; sleep 10
PL=$R/segmented.proxy.log; rm -f $PL*; (cd /tmp && nohup bun $ROOT/scripts/gpui/android-proxy.ts 100.64.84.41:3815 100.64.84.41:3814 $PL >/dev/null 2>&1 & echo $! > $H/proxy.pid); sleep 1
$E adb uninstall dev.backplane.mobile.gpui >/dev/null 2>&1; $E adb install $APK | tail -1; $E adb shell pm clear dev.backplane.mobile.gpui >/dev/null; $E adb shell pm grant dev.backplane.mobile.gpui android.permission.POST_NOTIFICATIONS
PORT=3815 TOKENFILE=$HD/token $ROOT/scripts/gpui/android-pair.sh >/dev/null
for i in $(seq 1 40); do $U has Skip exact && break; done; xy=$($U find Skip exact) && $E adb shell input tap $xy; sleep 2
set -- $($U find "Seed thread 1"); $E adb shell input tap 450 $2; sleep 8
$E adb exec-out screencap -p > $S/segmented-thread.png
python3 - "$PL" <<'P' | tee -a $OUT
import glob,sys,cbor2,re
log=sys.argv[1]; KEYS=["$","t","id","m","p","ok","error","items","thread","msg","role","text","at","state","project","title","kind","summary","tone","act","session","model","provider","runtime","interaction","since","seq","version","info","path","value","key","task","parent","child","depth","branch","env","worktree","root","pinned","settled","name","result","content","type","cid","count","manual","archived"]
WORDS=["MessagePosted","TurnChanged","ActivityLogged","changes","delta","reply"]
def t_of(o):
    for k,v in o.items():
        if k==1 or k=="t": return WORDS[v.value] if isinstance(v,cbor2.CBORTag) and v.tag==6 and v.value<len(WORDS) else v
ctl={}; big=[]
for f in sorted(glob.glob(log+".*s2c_*.bin"),key=lambda x:int(re.search(r'_(\d+)\.bin',x).group(1))):
    d=open(f,'rb').read()
    if len(d)<=5: continue
    try: o=cbor2.loads(d)
    except Exception: continue
    if not isinstance(o,dict): continue
    t=t_of(o)
    if t=='view.control': ctl.setdefault(1,[]).append(len(d))
    elif len(d)>20000: big.append((t,len(d)))
print("view.control parts:", {k:(len(v),sum(v)) for k,v in ctl.items()}, "| frames over 20 KB (type, bytes):", big)
P
say "first line of the big message visible: $($U has "line 0000" && echo yes || echo no); end mark visible: $($U has endmark && echo yes || echo no); View failed banner: $($U has 'View failed' && echo YES || echo no); webview nodes: $($U count android.webkit.WebView)"
say "turns: TurnUsage events=$(grep -c TurnUsage $HD/events.jsonl); assistant msg ids: $(grep -h '"role":"assistant"' $HD/events.jsonl | grep -o '"msg":"[^"]*"' | head -1)"
