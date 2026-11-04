#!/bin/sh
# Runs inside a Boat sandbox at the repo root. Usage: boat-run.sh KERNEL N REPS
# Builds KERNEL (native/bend/KERNEL.bend) as C -O2/-O3 and as JS, runs both and
# the TS mirror (3 fresh processes each), writes everything to out/.
set -eu
K=$1; N=$2; REPS=$3; IN=${4:-1}
export BEND_NO_TELEMETRY=1
B=native/bend; O=out/$K; mkdir -p $O
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, $(uname -m), clang $(clang --version | head -1), bun $(bun --version), bend $(bend version)" | tee $O/machine.txt
m() { /usr/bin/time -o $O/.t -f "%e s %M KiB" "$@" > $O/last.log 2>&1 || { echo "BUILD FAILED: $*"; head -60 $O/last.log; cat $O/.t; exit 1; }; cat $O/.t; }
{ printf "emit-c:  "; m bend $B/$K.bend -o $O/$K.c; } >> $O/build.txt || { cat $O/build.txt; exit 1; }
{ printf "emit-js: "; m bend $B/$K.bend -o $O/$K.js; } >> $O/build.txt || { cat $O/build.txt; exit 1; }
for L in O2 O3; do { printf "clang-$L: "; m clang -std=c11 -$L $O/$K.c -lpthread -lm -o $O/$K.$L; } >> $O/build.txt || { cat $O/build.txt; exit 1; }; done; cat $O/build.txt
ls -l $O/$K.c $O/$K.js $O/$K.O2 $O/$K.O3 | tee -a $O/build.txt
# the shipped hub's JS: bun-minified plus scripts/js-native.py's native String/Map/List primitives
if bun build --minify-whitespace --minify-syntax --keep-names --target=bun $O/$K.js --outfile $O/$K.hub.js >$O/hub.log 2>&1 && python3 scripts/js-native.py $O/$K.hub.js >>$O/hub.log 2>&1; then HUBJS=1; else HUBJS=0; echo "hub-js variant failed"; cat $O/hub.log | head; fi
ulimit -s 1048576 || true
run() { # name, then command
  nm=$1; shift
  for p in 1 2 3; do
    /usr/bin/time -f "rss_kib %M" "$@" > $O/run.$nm.$p.txt 2> $O/run.$nm.$p.err || { echo "FAILED $nm $p"; tail -5 $O/run.$nm.$p.err; }
    cat $O/run.$nm.$p.err >> $O/run.$nm.$p.txt
  done
}
run native-O2 $O/$K.O2 $N $REPS $IN
run native-O3 $O/$K.O3 $N $REPS $IN
run native-O3-1thread $O/$K.O3 --threads 1 $N $REPS $IN
run bendjs env BUN_JSC_maxPerThreadStackUsage=536870912 bun $O/$K.js $N $REPS $IN
[ $HUBJS = 1 ] && run bendjs-hub env BUN_JSC_maxPerThreadStackUsage=536870912 bun $O/$K.hub.js $N $REPS $IN
run ts bun test/gpui/bend/ts/$K.ts $N $REPS $IN
for f in $O/run.*.1.txt; do echo "== $f"; cat $f; done
