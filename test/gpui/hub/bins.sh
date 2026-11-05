#!/bin/sh
# BOAT ONLY: the hub binaries for the parity reruns: out/bins/backplane-hub-full (the whole hub, what ships) and window.
# The build uses one target dir outside out/ ($HOME/bins-target), so dependencies carry between runs in a warm sandbox
# and only the binary and logs go into out/bins.
case "$HOME" in /home/h|/home/h/*) echo "refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; O=$R/out/bins; mkdir -p $O; export BEND_NO_TELEMETRY=1
T=$HOME/bins-target
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -1; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
(cd native/host && rm -f $T/release/backplane-hub && CARGO_TARGET_DIR=$T cargo build --release --bin backplane-hub > $O/build-full.log 2>&1); bf=$?
grep -E '^error|Finished' $O/build-full.log | head -8; [ $bf -eq 0 ] && { cp $T/release/backplane-hub $O/backplane-hub-full || bf=1; }; [ $bf -eq 0 ] || exit $bf
rm -f $O/window.c; ls -l $O
sh test/gpui/hub/copy_gate.sh $O/backplane-hub-full > $O/copy_gate.txt 2>&1; cg=$?; cat $O/copy_gate.txt
(cd native/host && CARGO_TARGET_DIR=$T cargo test --release copy:: > $O/copy_unit_full.txt 2>&1); ct=$?; echo "exit $ct" >> $O/copy_unit_full.txt; grep -E "^test |test result|^error|panicked|exit " $O/copy_unit_full.txt | tee $O/copy_unit.txt; exit $(( cg | ct ))
