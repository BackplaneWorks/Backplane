#!/bin/sh
# inside Boat: status over 20000 threads, 1000 iterations, native -O3 (bigger live heap)
set -eu
export BEND_NO_TELEMETRY=1
O=out/loopbig; mkdir -p $O
bend native/bend/loop.bend -o $O/loop.c 2>&1 | head -20
clang -std=c11 -O3 $O/loop.c -lpthread -lm -o $O/loop.O3
echo "machine: $(nproc) vCPU, $(uname -m)" > $O/machine.txt
/usr/bin/time -f "rss_kib %M wall_s %e" $O/loop.O3 1 1000 20000 > $O/native.1.txt 2> $O/native.1.err || tail -3 $O/native.1.err
cat $O/native.1.err; head -3 $O/native.1.txt
