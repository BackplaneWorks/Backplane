#!/bin/sh
# On the Mac: time to a ready thread after a relaunch that restores a persisted draft of 0 / 20 / 40 / 60 / 100 KB (BPPerf modelReady) and any view failure.
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer HUBURL=http://backplane.tail9e0cf2.ts.net:3811/ TOKFILE=~/bp-gpui-ios/seed7-token
U=$1; T=$2
xcrun simctl boot $U 2>/dev/null
for n in ${SIZES:-0 20 40 60 100}; do
  xcrun simctl uninstall $U dev.backplane.gpui
  if [ $n -gt 0 ]; then
    python3 -c "print(('lorem ipsum dolor sit amet ' * 5000)[:${n}000], end='')" > /tmp/ld$n.txt
    sh ~/bp-gpui-ios/ios-shot.sh $U rd$n-a 20 "ob-skip;;@type=$(cat /tmp/ld$n.txt)" $T > /dev/null
    xcrun simctl terminate $U dev.backplane.gpui; sleep 2
  fi
  SIMCTL_CHILD_BACKPLANE_PERFORMANCE=1 sh ~/bp-gpui-ios/ios-shot.sh $U rd$n-b 50 "ob-skip" $T > /dev/null
  xcrun simctl spawn $U log show --last 90s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "BPPerf|view failed" | sed 's/^.*Backplane\[[0-9:a-f]*\] ([A-Za-z]*) //' > ~/bp-gpui-ios/shots/rd$n.log
  # the hub's own view of it: the slowest thin.rpc / hub.rpc so far (cumulative max, sizes ascend), from its /debug/perf
  curl -s -m 5 -H "Authorization: Bearer $(cat ~/bp-gpui-ios/seed7-token)" http://backplane.tail9e0cf2.ts.net:3811/debug/perf | grep -E "^(thin.rpc|hub.rpc|thin.read.page)" | awk -v n=$n '{printf "  hub perf @%sKB: %s n=%s max=%sms\n", n, $1, $2, $7}'
  python3 - $n <<'PY'
import re,sys
n=sys.argv[1]; rows=[]; fails=0
for l in open('/Users/h/bp-gpui-ios/shots/rd%s.log'%n):
    if 'view failed' in l: fails+=1
    m=re.match(r"BPPerf (\d+) (\d+) (\d+) (\d+) (\d+) (\d+)",l)
    if m: rows.append(tuple(map(int,m.groups())))
rdy=[round((r[3]-r[2])/1e6) for r in rows if r[0]==7]
print("draft %sKB: modelReady ms %s, view failures %d"%(n,rdy[:4],fails))
PY
done
xcrun simctl shutdown $U
