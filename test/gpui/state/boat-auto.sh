#!/bin/sh
# Boat: the autosettle probe, Bend's Auto.changes (native/bend/autosettle_probe.bend, C -O2) against the Rust twin (state-probe) on the synthetic
# logs in test/gpui/state/auto-logs, at each time in auto.now. Artifacts: out/auto/{result.txt,test.txt,*.out}
set -u
export BEND_NO_TELEMETRY=1 PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-state
O=out/auto; mkdir -p $O
echo "bend $(bend version) clang $(clang --version | head -1)" > $O/result.txt
( ulimit -s 1048576; /usr/bin/time -f "bend emit %e s %M KiB" bend native/bend/autosettle_probe.bend -o $O/probe.c > $O/emit.txt 2>&1; echo emit-exit $? >> $O/emit.txt )
tail -3 $O/emit.txt; grep -q 'emit-exit 0' $O/emit.txt || { head -40 $O/emit.txt; exit 1; }
clang -std=c11 -O2 $O/probe.c -lpthread -lm -o $O/probe > $O/clang.txt 2>&1 || { head -20 $O/clang.txt; exit 1; }
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "
  cargo test --release --manifest-path native/host/Cargo.toml --bin state-probe --bin rust-digest > $O/test.txt 2>&1; echo test-exit \$? >> $O/test.txt
  cargo build --release --manifest-path native/host/Cargo.toml --bin state-probe > $O/build.txt 2>&1; echo build-exit \$? >> $O/build.txt
"
tail -4 $O/test.txt; grep -E 'test-exit|build-exit' $O/test.txt $O/build.txt
norm() { python3 -c "
import sys,json
d=json.load(sys.stdin)
for k in ('tick','meta','quiet','live','end','wakes','upgrade','reopen'):
    for x in sorted(json.dumps(e,sort_keys=True) for e in d[k]): print(k,x)"; }
fail=0; total=0; nonempty=0
for f in test/gpui/state/auto-logs/auto-*.jsonl; do n=$(basename $f .jsonl)
  for now in $(cat test/gpui/state/auto-logs/auto.now); do
   for B in test/gpui/state/auto-logs/auto.batch test/gpui/state/auto-logs/auto.batch2 test/gpui/state/auto-logs/auto.batch3 test/gpui/state/auto-logs/auto.batch4; do
    b=$(basename $B)
    ( ulimit -s 1048576; $O/probe $now $B $f ) | norm > $O/$n.$now.$b.bend
    [ -s $O/$n.$now.$b.bend ] && nonempty=$((nonempty+1))
    for lean in 0 1; do
      LEAN=$lean target-state/release/state-probe $now $B $f | norm > $O/$n.$now.$b.rust$lean
      total=$((total+1))
      if cmp -s $O/$n.$now.$b.bend $O/$n.$now.$b.rust$lean; then :; else fail=$((fail+1)); echo "DIFF lean=$lean $n $now $b: bend $(wc -l < $O/$n.$now.$b.bend) rust $(wc -l < $O/$n.$now.$b.rust$lean)" | tee -a $O/result.txt; diff $O/$n.$now.$b.bend $O/$n.$now.$b.rust$lean | head -8 | tee -a $O/result.txt; fi
    done
   done
  done
done
for f in test/gpui/state/auto-logs/auto-base.jsonl test/gpui/state/auto-logs/auto-notify.jsonl; do n=$(basename $f .jsonl)
  for now in 1794000700 1794020000 1794200000; do
   for B in test/gpui/state/auto-logs/end-*.batch; do
    b=$(basename $B)
    ( ulimit -s 1048576; $O/probe $now $B $f ) | norm > $O/$n.$now.$b.bend
    [ -s $O/$n.$now.$b.bend ] && nonempty=$((nonempty+1))
    for lean in 0 1; do
      LEAN=$lean target-state/release/state-probe $now $B $f | norm > $O/$n.$now.$b.rust$lean
      total=$((total+1))
      if cmp -s $O/$n.$now.$b.bend $O/$n.$now.$b.rust$lean; then :; else fail=$((fail+1)); echo "DIFF lean=$lean $n $now $b: bend $(wc -l < $O/$n.$now.$b.bend) rust $(wc -l < $O/$n.$now.$b.rust$lean)" | tee -a $O/result.txt; diff $O/$n.$now.$b.bend $O/$n.$now.$b.rust$lean | head -8 | tee -a $O/result.txt; fi
    done
   done
  done
done
echo "autosettle probe: $total runs (full and lean state each), $nonempty with changes, $fail differ" | tee -a $O/result.txt
[ $fail = 0 ]
