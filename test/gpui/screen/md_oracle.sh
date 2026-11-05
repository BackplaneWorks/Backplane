#!/bin/sh
# BOAT ONLY (synthetic corpus, no store copy): Bend's Md.render + Mob.nodes against native/host/src/md on the same strings.
# usage from the repo root: N=3000 SEED=1 sh test/gpui/screen/md_oracle.sh   (artifacts out/md/{corpus.json,bend.txt,rust.txt,diff.txt})
set -u
O=out/md; mkdir -p $O; N=${N:-3000}; SEED=${SEED:-1}
export PATH="$HOME/.cargo/bin:$PATH" BEND_NO_TELEMETRY=1
python3 test/gpui/screen/md_copy_check.py || exit 4
python3 test/gpui/screen/md_corpus.py $N $SEED > $O/corpus.json
(cd native/host && cargo test --release md:: 2>&1 | grep -E "^test |test result|error" | tail -20) | tee $O/unit.txt
(cd native/host && cargo build --release --bin md-check 2>&1 | grep -E "^error|Finished") || exit 5
native/host/target/release/md-check $O/corpus.json > $O/rust.txt
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- sh -c "bend test/gpui/screen/md_dump.bend $O/corpus.json > $O/bend.txt 2> $O/bend.err"
echo "bend lines $(wc -l < $O/bend.txt) rust lines $(wc -l < $O/rust.txt); bend.err:"; head -5 $O/bend.err
python3 test/gpui/screen/md_compare.py $O/corpus.json $O/bend.txt $O/rust.txt > $O/diff.txt; tail -3 $O/diff.txt; head -40 $O/diff.txt
grep -q "^MISMATCHES 0" $O/diff.txt
