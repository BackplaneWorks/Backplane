#!/bin/sh
# Runs in a Boat sandbox at the repo root (scripts/boat-job.sh): check, emit C, clang -O2 the
# slim policy server. Artifacts: out/slim/bp-policy (stripped) and build.txt (time, peak memory).
set -u
export BEND_NO_TELEMETRY=1
O=out/slim; mkdir -p $O
SRC=${SRC:-native/bend/slim_serve.bend}
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, clang $(clang --version | head -1), bend $(bend version), src $SRC" | tee $O/build.txt
step() { n=$1; shift; /usr/bin/time -o $O/.t -f "$n: %e s %M KiB exit %x" systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@" > $O/$n.log 2>&1; rc=$?; cat $O/.t | tee -a $O/build.txt; [ $rc = 0 ] || { tail -40 $O/$n.log; exit $rc; }; }
step check bend $SRC --check-only
[ "${CHECK_ONLY:-}" = 1 ] && exit 0
step emit-c bend $SRC -o $O/policy.c
ls -l $O/policy.c | tee -a $O/build.txt
step clang-O2 clang -std=c11 -O2 $O/policy.c -lpthread -lm -o $O/bp-policy
strip -o $O/bp-policy.stripped $O/bp-policy; ls -l $O/bp-policy $O/bp-policy.stripped | tee -a $O/build.txt
ldd $O/bp-policy | tee -a $O/build.txt
rm -f $O/policy.c.gz; gzip -k $O/policy.c 2>/dev/null; rm -f $O/policy.c
