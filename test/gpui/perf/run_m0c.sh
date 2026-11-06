#!/bin/bash
cd /home/h/Documents/i2cjak/backplane-gpui/test/gpui/perf; D=../../../build/gpui/results/perf
PID=3569861
# wait for the first-start import to finish (hello answered) or the hub to die (OOM at the cap)
for i in $(seq 900); do
  [ -d /proc/$PID ] || break
  curl -s -m 1 -o /dev/null http://100.64.84.41:3830/hello && break
  sleep 4
done
if [ -d /proc/$PID ]; then
  sleep 30; grep VmRSS /proc/$PID/status > $D/m0/first-after-hello.txt; grep VmHWM /proc/$PID/status >> $D/m0/first-after-hello.txt
  kill $PID; for i in $(seq 100); do [ -d /proc/$PID ] || break; sleep 1; done
  echo IMPORT_OK
  for ph in later steady load; do bun m0_baseline.mjs $ph > $D/m0.$ph.stdout 2>&1; echo $ph rc=$?; done
else
  echo IMPORT_HUB_DIED; tail -5 $D/m0/hub-first.log | cut -c1-200
fi
echo M0CDONE
