#!/bin/sh
# Boat: compile the already emitted policy C (build/gpui/slim/policy.c.gz) with memory-lean clang variants.
set -u
O=out/slim; mkdir -p $O; gunzip -c build/gpui/slim/policy.c.gz > $O/policy.c
cstep() { n=$1; shift
  ( systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@" > $O/$n.log 2>&1; echo $? > $O/$n.rc ) &
  w=$!; t0=$(date +%s); peak=0
  while kill -0 $w 2>/dev/null; do
    r=$(ps -eo rss=,comm= | awk '$2=="clang"||$2=="clang-18"||$2=="cc1"{if($1>m)m=$1}END{print m+0}')
    [ "$r" -gt "$peak" ] && peak=$r
    echo "$n t=$(( $(date +%s) - t0 )) rss_kib=$r" >> $O/$n.samples; sleep 5
  done
  rc=$(cat $O/$n.rc); echo "$n: $(( $(date +%s) - t0 )) s peak_kib=$peak exit $rc" | tee -a $O/build.txt; return $rc; }
cstep Os clang -std=c11 -Os $O/policy.c -lpthread -lm -o $O/bp-policy-Os && strip -o $O/bp-policy-Os.stripped $O/bp-policy-Os && ls -l $O/bp-policy-Os.stripped | tee -a $O/build.txt
cstep O1lean clang -std=c11 -O1 -fno-inline-functions -fno-vectorize -fno-slp-vectorize -fno-unroll-loops $O/policy.c -lpthread -lm -o $O/bp-policy-O1lean && strip -o $O/bp-policy-O1lean.stripped $O/bp-policy-O1lean && ls -l $O/bp-policy-O1lean.stripped | tee -a $O/build.txt
cstep O2lean clang -std=c11 -O2 -fno-vectorize -fno-slp-vectorize -fno-unroll-loops -mllvm -inline-threshold=25 $O/policy.c -lpthread -lm -o $O/bp-policy-O2lean && strip -o $O/bp-policy-O2lean.stripped $O/bp-policy-O2lean && ls -l $O/bp-policy-O2lean.stripped | tee -a $O/build.txt
rm -f $O/policy.c
