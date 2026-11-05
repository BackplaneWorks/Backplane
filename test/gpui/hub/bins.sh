#!/bin/sh
# BOAT ONLY: the hub binaries for the parity reruns: out/bins/backplane-hub-full (all streams), backplane-hub (core), window
# Builds share one target dir outside out/ (dependencies are kept between the two builds and between runs in a warm sandbox);
# only the binaries and logs go into out/bins. The core copy lives outside the repo, so the source is never edited in place.
case "$HOME" in /home/h|/home/h/*) echo "refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; O=$R/out/bins; mkdir -p $O; export BEND_NO_TELEMETRY=1
T=$HOME/bins-target; C=$HOME/bins-core
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -1; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
(cd native/host && rm -f $T/release/backplane-hub && CARGO_TARGET_DIR=$T cargo build --release --bin backplane-hub > $O/build-full.log 2>&1); bf=$?
grep -E '^error|Finished' $O/build-full.log | head -8; [ $bf -eq 0 ] && { cp $T/release/backplane-hub $O/backplane-hub-full || bf=1; }; [ $bf -eq 0 ] || exit $bf
rm -rf $C && mkdir -p $C/native/host $C/src/core && cp src/core/cbor.bend $C/src/core/ && (cd native/host && tar --exclude=./target -cf - .) | (cd $C/native/host && tar -xf -) && (cd $C/native/host && rm -rf src/agents src/files src/kicad && sed -i 's/mod agents; //; s/mod files; //; s/mod kicad; //; /agents::Agents::start/d; /files::start/d' src/main.rs && rm -f $T/release/backplane-hub && CARGO_TARGET_DIR=$T cargo build --release --bin backplane-hub > $O/build-core.log 2>&1); bc=$?
grep -E '^error|Finished' $O/build-core.log | head -8; [ $bc -eq 0 ] && { cp $T/release/backplane-hub $O/backplane-hub || bc=1; }; [ $bc -eq 0 ] || exit $bc
rm -f $O/window.c; ls -l $O
sh test/gpui/hub/copy_gate.sh $O/backplane-hub-full > $O/copy_gate.txt 2>&1; cg=$?; cat $O/copy_gate.txt
(cd native/host && cargo test --release copy:: > $O/copy_unit_full.txt 2>&1); ct=$?; echo "exit $ct" >> $O/copy_unit_full.txt; grep -E "^test |test result|^error|panicked|exit " $O/copy_unit_full.txt | tee $O/copy_unit.txt; exit $(( cg | ct ))
