#!/bin/sh
# Runs in a Boat sandbox at the repo root (scripts/boat-job.sh): check, emit C, clang -O2 the
# slim policy server. Artifacts: out/slim/bp-policy (stripped) and build.txt (time, peak memory).
set -u
export BEND_NO_TELEMETRY=1
O=out/slim; mkdir -p $O
SRC=${SRC:-native/bend/slim_serve.bend}
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, clang $(clang --version | head -1), bend $(bend version), src $SRC" | tee $O/build.txt
step() { n=$1; shift; /usr/bin/time -o $O/.t -f "$n: %e s %M KiB exit %x" systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@" > $O/$n.log 2>&1; rc=$?; cat $O/.t | tee -a $O/build.txt; [ $rc = 0 ] || { tail -40 $O/$n.log; return $rc; }; }
step check bend $SRC --check-only || exit 1
[ "${CHECK_ONLY:-}" = 1 ] && exit 0
# emit peaks at 7.8-8.4 GB against the 8 GiB cap and varies run to run: make JSC collect earlier, retry
export BUN_JSC_forceRAMSize=${EMIT_RAM:-5368709120}
for try in 1 2 3; do
  step emit-c-$try bend $SRC -o $O/policy.c && break
  [ $try = 3 ] && exit 1
done
ls -l $O/policy.c | tee -a $O/build.txt
# clang in its own capped scope, its cc1 child's RSS sampled every 5 s (the scope's OOM stop shows only as "Terminated")
cstep() { n=$1; shift
  ( systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@" > $O/$n.log 2>&1; echo $? > $O/$n.rc ) &
  w=$!; t0=$(date +%s); peak=0
  while kill -0 $w 2>/dev/null; do
    r=$(ps -eo rss=,comm= | awk '$2=="clang"||$2=="clang-18"||$2=="cc1"{if($1>m)m=$1}END{print m+0}')
    [ "$r" -gt "$peak" ] && peak=$r
    echo "$n t=$(( $(date +%s) - t0 )) rss_kib=$r" >> $O/$n.samples; sleep 5
  done
  rc=$(cat $O/$n.rc); echo "$n: $(( $(date +%s) - t0 )) s peak_kib=$peak exit $rc" | tee -a $O/build.txt
  [ $rc = 0 ] || tail -5 $O/$n.log; return $rc; }
LVL=none
for l in ${LEVELS:-O0 O1 O2}; do
  cstep clang-$l clang -std=c11 -$l $O/policy.c -lpthread -lm -o $O/bp-policy-$l && { LVL=$l; cp $O/bp-policy-$l $O/bp-policy; }
done
[ -f $O/bp-policy ] || exit 1
echo "opt level: $LVL" | tee -a $O/build.txt
strip -o $O/bp-policy.stripped $O/bp-policy; ls -l $O/bp-policy $O/bp-policy.stripped | tee -a $O/build.txt
ldd $O/bp-policy | tee -a $O/build.txt
rm -f $O/policy.c.gz; gzip -k $O/policy.c 2>/dev/null; rm -f $O/policy.c
