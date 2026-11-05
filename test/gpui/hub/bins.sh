#!/bin/sh
# BOAT ONLY: the hub binaries for the parity reruns: out/bins/backplane-hub-full (all streams), backplane-hub (core), window
case "$HOME" in /home/h|/home/h/*) echo "refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; O=$R/out/bins; mkdir -p $O; export BEND_NO_TELEMETRY=1
bend native/bend/window.bend -o $O/window.c 2>&1 | tail -1; clang -std=c11 -O2 $O/window.c -lpthread -lm -o $O/window || exit 1
(cd native/host && cargo build --release --bin backplane-hub 2>&1 | grep -E '^error|Finished' | head -8; cp target/release/backplane-hub $O/backplane-hub-full)
(cd native/host && rm -rf src/agents src/files src/kicad && sed -i 's/mod agents; //; s/mod files; //; s/mod kicad; //; /agents::Agents::start/d; /files::start/d' src/main.rs && cargo build --release --bin backplane-hub 2>&1 | grep -E '^error|Finished' | head -8; cp target/release/backplane-hub $O/backplane-hub)
rm -f $O/window.c; ls -l $O
