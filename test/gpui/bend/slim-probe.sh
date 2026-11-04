#!/bin/sh
# Runs in a Boat sandbox at the repo root: can the real fold and screen model
# reach machine code under the 8 GiB cap?
set -u
export BEND_NO_TELEMETRY=1
O=out/slim; mkdir -p $O
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, clang $(clang --version | head -1), bend $(bend version)" | tee $O/build.txt
step() { n=$1; shift; /usr/bin/time -o $O/.t -f "$n: %e s %M KiB exit %x" systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@" > $O/$n.log 2>&1; rc=$?; cat $O/.t | tee -a $O/build.txt; [ $rc = 0 ] || { tail -30 $O/$n.log; exit $rc; }; }
step check bend native/bend/slim.bend --check-only
step emit-c bend native/bend/slim.bend -o $O/slim.c
ls -l $O/slim.c | tee -a $O/build.txt
step clang-O2 clang -std=c11 -O2 $O/slim.c -lpthread -lm -o $O/slim
strip -o $O/slim.stripped $O/slim; ls -l $O/slim $O/slim.stripped | tee -a $O/build.txt
ldd $O/slim | tee -a $O/build.txt
