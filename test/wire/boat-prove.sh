#!/bin/sh
# Boat: the dropped-link proof against the Rust hub (stand-in agents, 20 streaming), hard cap per hub inside reconnect_prove.mjs.
# usage: from the repo root: sh test/wire/boat-prove.sh   (the hub is this worktree's native/host, built on the box: a hub built on the desktop
# is linked against a newer glibc than a Boat box has). Exit: 0 PASS, 1 the proof failed, 3 the hub did not build, 124 timed out. Results: the job folder scripts/boat-job.sh names, artifacts/prove (rust.txt, rust.json, build.log).
set -eu
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/prove \
  -- sh -c 'which bun >/dev/null 2>&1 || export PATH="$HOME/.bun/bin:$PATH"; which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }; mkdir -p out/prove; (cd native/host && cargo build --release > ../../out/prove/build.log 2>&1) || { tail -20 out/prove/build.log; echo "the hub did not build (exit 3, not a proof result)"; exit 3; }; BP_ROOT=$PWD bun test/wire/reconnect_prove.mjs $PWD/native/host/target/release/backplane-hub 3875 20 out/prove/rust.json > out/prove/rust.txt 2>&1; rc=$?; echo rc=$rc >> out/prove/rust.txt; grep -E "PASS|FAIL|RESULT|rc=|Error" out/prove/rust.txt; exit $rc'
