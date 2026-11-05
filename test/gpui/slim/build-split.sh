#!/bin/sh
# Boat: bend emit -> cc-split.py (16 units) -> one clang -O2 at a time under the 8 GiB cap -> link.
# Restored from the C-hub era (3de248dd5b^:scripts/cc-split.py). Env: SRC, UNITS, OPT, PRE (policy.c.gz to skip the emit).
set -u
export BEND_NO_TELEMETRY=1
O=out/split; mkdir -p $O
SRC=${SRC:-native/bend/slim_serve.bend}; UNITS=${UNITS:-16}; OPT=${OPT:--O2}
now() { date +%s; }
T0=$(now)
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, clang $(clang --version | head -1), bend $(bend version), src $SRC units $UNITS opt $OPT" | tee $O/build.txt
capped() { systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- "$@"; }
# peak of the scope's tree sampled every 0.5 s
peakof() { f=$1; shift; ( "$@" ) & w=$!; p=0; while kill -0 $w 2>/dev/null; do r=$(ps -eo rss=,comm= | awk '$2=="clang"||$2=="clang-18"||$2=="cc1"||$2=="bun"||$2=="bend"{s+=$1}END{print s+0}'); [ "$r" -gt "$p" ] && p=$r; sleep 0.5; done; wait $w; rc=$?; echo $p > $f; return $rc; }
if [ -n "${PRE:-}" ] && [ -f "$PRE" ]; then gunzip -c "$PRE" > $O/all.c; echo "emit: reused $PRE" | tee -a $O/build.txt; EMIT_S=0; EMIT_KB=0
else
  export BUN_JSC_forceRAMSize=${EMIT_RAM:-5368709120}
  t=$(now); peakof $O/emit.peak capped bend $SRC -o $O/all.c > $O/emit.log 2>&1 || { tail -20 $O/emit.log; exit 1; }
  EMIT_S=$(( $(now) - t )); EMIT_KB=$(cat $O/emit.peak); echo "emit: ${EMIT_S}s peak_kib=$EMIT_KB" | tee -a $O/build.txt
fi
ls -l $O/all.c | tee -a $O/build.txt
t=$(now); python3 test/gpui/slim/cc-split.py $O/all.c $O $UNITS | tee -a $O/build.txt; SPLIT_S=$(( $(now) - t ))
ls -l $O/u*.c | tee -a $O/build.txt
CCMAX=0; t=$(now)
for c in $(ls -S $O/u*.c); do
  o=${c%.c}.o; t1=$(now)
  peakof $c.peak capped clang -std=c11 $OPT -w -c $c -o $o > $c.log 2>&1 || { echo "cc failed $c"; tail -20 $c.log; exit 1; }
  k=$(cat $c.peak); [ "$k" -gt "$CCMAX" ] && CCMAX=$k
  echo "cc $c: $(( $(now) - t1 )) s peak_kib=$k" | tee -a $O/build.txt
done
CC_S=$(( $(now) - t )); t=$(now)
capped clang $OPT $O/u*.o -o $O/bp-policy -lpthread -lm || exit 1
LINK_S=$(( $(now) - t ))
strip -o $O/bp-policy.stripped $O/bp-policy
ls -l $O/bp-policy $O/bp-policy.stripped | tee -a $O/build.txt
echo "TOTAL $(( $(now) - T0 )) s: emit ${EMIT_S}s (peak ${EMIT_KB} KiB) split ${SPLIT_S}s cc ${CC_S}s (largest unit peak ${CCMAX} KiB) link ${LINK_S}s" | tee -a $O/build.txt
rm -f $O/u*.c $O/u*.o
