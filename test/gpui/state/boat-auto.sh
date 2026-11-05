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
for k in ('tick','meta','quiet'):
    for x in sorted(json.dumps(e,sort_keys=True) for e in d[k]): print(k,x)"; }
fail=0; total=0; nonempty=0
for f in test/gpui/state/auto-logs/auto-*.jsonl; do n=$(basename $f .jsonl)
  for now in $(cat test/gpui/state/auto-logs/auto.now); do
    B=test/gpui/state/auto-logs/auto.batch
    ( ulimit -s 1048576; $O/probe $now $B $f ) | norm > $O/$n.$now.bend
    target-state/release/state-probe $now $B $f | norm > $O/$n.$now.rust
    total=$((total+1)); [ -s $O/$n.$now.bend ] && nonempty=$((nonempty+1))
    if cmp -s $O/$n.$now.bend $O/$n.$now.rust; then :; else fail=$((fail+1)); echo "DIFF $n $now: bend $(wc -l < $O/$n.$now.bend) rust $(wc -l < $O/$n.$now.rust)" | tee -a $O/result.txt; diff $O/$n.$now.bend $O/$n.$now.rust | head -6 | tee -a $O/result.txt; fi
  done
done
echo "autosettle probe: $total runs, $nonempty with changes, $fail differ" | tee -a $O/result.txt
[ $fail = 0 ]
