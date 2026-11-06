#!/bin/sh
# Boat: the policy core on a 1M-line synthetic log with 6,666 threads: fold time, RSS added, tick duration (lean and full). Artifacts: out/bench/bench.txt
set -u
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=$PWD/target-bench
O=out/bench; mkdir -p $O
python3 test/gpui/state/gen_big.py /tmp/big.jsonl 1000000 6666 > $O/gen.txt
python3 test/gpui/state/gen_big.py /tmp/big-policy.jsonl 1000000 6666 policy >> $O/gen.txt
ls -l /tmp/big.jsonl | tee -a $O/gen.txt
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "
  cargo test --release --manifest-path native/host/Cargo.toml --bin rust-digest --bin state-probe > $O/test.txt 2>&1; echo test-exit \$? >> $O/test.txt
  cargo build --release --manifest-path native/host/Cargo.toml --bin state-bench > $O/build.txt 2>&1; echo build-exit \$? >> $O/build.txt
"
grep -E 'test result|FAILED|failed|test-exit' $O/test.txt | tail -5; tail -2 $O/build.txt
for mode in lean full; do target-bench/release/state-bench /tmp/big.jsonl $mode | tee -a $O/bench.txt; done
echo "-- with the policy keys present on every thread (settle., autosettle., liveness., alert., bg., watch., launch., thread.parent., wakes)" | tee -a $O/bench.txt
for mode in lean full; do target-bench/release/state-bench /tmp/big-policy.jsonl $mode | tee -a $O/bench.txt; done
grep -q 'test-exit 0' $O/test.txt && grep -q 'build-exit 0' $O/build.txt
