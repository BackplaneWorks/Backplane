#!/bin/sh
# inside Boat: try to emit the whole hub to C under the sandbox's memory
export BEND_NO_TELEMETRY=1
O=out/hub; mkdir -p $O
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, swap $(free -m | awk '/Swap/{print $2}') MiB" | tee $O/machine.txt
ulimit -s 1048576 || true
/usr/bin/time -f "emit-c exit=%x wall=%e s peakRSS=%M KiB" -o $O/time.txt timeout 1300 bend src/server/main.bend -o $O/hub.c > $O/emit.log 2>&1
echo "rc=$?" >> $O/time.txt
cat $O/time.txt; head -60 $O/emit.log; ls -l $O
dmesg 2>/dev/null | tail -3
