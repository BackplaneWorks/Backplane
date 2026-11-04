#!/bin/sh
# inside Boat: build loop (O3) and run it for status and history, native and bun-JS
set -eu
export BEND_NO_TELEMETRY=1
O=out/loop; mkdir -p $O
bend native/bend/loop.bend -o $O/loop.c 2>&1 | head -40
clang -std=c11 -O3 $O/loop.c -lpthread -lm -o $O/loop.O3
bend native/bend/loop.bend -o $O/loop.js 2>&1 | head -5
echo "machine: $(nproc) vCPU, $(uname -m), clang $(clang --version|head -1)" > $O/machine.txt
ulimit -s 1048576 || true
for K in 1 2; do
  /usr/bin/time -f "rss_kib %M wall_s %e" $O/loop.O3 $K 2000 > $O/native.$K.txt 2> $O/native.$K.err || tail -3 $O/native.$K.err
  /usr/bin/time -f "rss_kib %M wall_s %e" env BUN_JSC_maxPerThreadStackUsage=536870912 bun $O/loop.js $K 2000 > $O/js.$K.txt 2> $O/js.$K.err || tail -3 $O/js.$K.err
done
cat $O/native.*.err $O/js.*.err
head -3 $O/native.1.txt; head -3 $O/js.1.txt
