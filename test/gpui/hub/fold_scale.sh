#!/bin/sh
# fold_scale.sh : time to listening and memory of backplane-hub + bp-policy folding the first N events of the store copy.
# Runs on the desktop (a finished binary), 6 GiB scope, no copy of the log: events.jsonl is a symlink.
R=/home/h/Documents/i2cjak/backplane-gpui
POL=$R/build/gpui/slim/bp-policy-serve-O2
OUT=$R/build/gpui/results/hub/fold-scale.txt; : > $OUT
for N in 25000 50000 100000 200000 400000; do
  H=$R/build/gpui/hub-run/copy-home-$N; rm -rf $H; mkdir -p $H; ln -s /home/h/.backplane-gpui-copy/events.jsonl $H/events.jsonl
  BACKPLANE_FOLD_LIMIT=$N timeout 400 systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 $R/build/gpui/hub/backplane-hub --home $H --port 3815 --policy-bin $POL > $H/log 2>&1 &
  T0=$(date +%s.%N); OK=0
  for i in $(seq 1 3000); do curl -s -m 1 http://127.0.0.1:3815/hello >/dev/null && { OK=1; break; }; sleep 0.1; done
  T1=$(date +%s.%N)
  PID=$(awk '{print $1}' $H/hub.lock 2>/dev/null)
  PP=$(pgrep -P "$PID" 2>/dev/null | head -1)
  echo "N=$N listening=$OK seconds=$(echo "$T1 - $T0" | bc) hub_rss_kb=$(awk '/VmRSS/{print $2}' /proc/$PID/status 2>/dev/null) hub_hwm_kb=$(awk '/VmHWM/{print $2}' /proc/$PID/status 2>/dev/null) policy_rss_kb=$(awk '/VmRSS/{print $2}' /proc/$PP/status 2>/dev/null) policy_hwm_kb=$(awk '/VmHWM/{print $2}' /proc/$PP/status 2>/dev/null)" >> $OUT
  kill $PID 2>/dev/null; sleep 2; pkill -P $$ 2>/dev/null; rm -rf $H
done
echo done >> $OUT
