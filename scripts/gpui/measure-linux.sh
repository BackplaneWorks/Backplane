#!/bin/sh
# Measures cold and warm builds on a Linux machine (run through scripts/gpui/bp-build.sh, never on the desktop).
# Cold = a fresh CARGO_TARGET_DIR; warm = touch one source file and rebuild in the same target. Each step under /usr/bin/time -v.
set -u
R=$PWD; O=$R/measure; mkdir -p $O; export PATH=$HOME/.cargo/bin:$PATH
S=$O/summary.txt; : > $S
echo "machine: $(hostname), $(nproc) cpus, $(free -g | sed -n 2p | awk '{print $2" GiB"}'), rustc $(rustc --version)" >> $S
echo "commit: $(git rev-parse --short HEAD 2>/dev/null || echo unknown)" >> $S
step() { name=$1; shift; /usr/bin/time -v -o $O/$name.time "$@" > $O/$name.log 2>&1; rc=$?
  wall=$(grep "Elapsed (wall clock)" $O/$name.time | sed 's/.*: //'); rss=$(grep "Maximum resident set size" $O/$name.time | sed 's/.*: //')
  echo "$name exit $rc wall $wall maxrss_KiB $rss" >> $S; }
# Rust hub, release
HC=$(mktemp -d); step hub-cold env CARGO_TARGET_DIR=$HC sh -c 'cd native/host && cargo build --release --bin backplane-hub'
touch native/host/src/main.rs; step hub-warm env CARGO_TARGET_DIR=$HC sh -c 'cd native/host && cargo build --release --bin backplane-hub'
# whole test suite of native/host (build of the test targets and the run)
TC=$(mktemp -d); step suite-cold env CARGO_TARGET_DIR=$TC sh -c 'cd native/host && cargo test'
touch native/host/src/main.rs; step suite-warm env CARGO_TARGET_DIR=$TC sh -c 'cd native/host && cargo test'
# GPUI desktop, release (only if its Linux dependencies are present)
if pkg-config --exists xkbcommon-x11 fontconfig; then
  GC=$(mktemp -d); step gpui-cold env CARGO_TARGET_DIR=$GC cargo build --release --manifest-path native/gpui/Cargo.toml
  touch native/gpui/src/main.rs; step gpui-warm env CARGO_TARGET_DIR=$GC cargo build --release --manifest-path native/gpui/Cargo.toml
else echo "gpui-cold not measured: Linux deps missing" >> $S; fi
rm -rf $HC $TC ${GC:-} 2>/dev/null
cat $S
