#!/bin/sh
# BOAT ONLY (fixture hub, stand-in agents, no store copy): record the `commands` scenario against the post-#960 JS hub (twice: a self-diff),
# build the Rust hub from this tree and replay the recording against it. Inputs staged by boat-parity-launch.sh.
# Artifacts: out/commands-parity/{self-diff.txt,replay-diff.txt,recJ,recJ2,replay,build.txt}
set -u
O=out/commands-parity; mkdir -p $O
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
chmod +x pstage/backplane-serve pstage/bp-policy-page pstage/window test/slim/check_agents.sh 2>/dev/null
rm -rf /tmp/parity-work  # a shared host keeps the last run: folders it made (proj/made, proj/fresh, .backplane/intent.md) must not be there
export BACKPLANE_PARITY_ROOT=$PWD PARITY_WORK=/tmp/parity-work
bun test/parity/record.mjs --bin $PWD/pstage/backplane-serve --out $O/recJ --port 3820 --tag A --only commands > $O/recJ.log 2>&1
bun test/parity/record.mjs --bin $PWD/pstage/backplane-serve --out $O/recJ2 --port 3822 --tag B --only commands > $O/recJ2.log 2>&1
bun test/parity/compare.mjs $O/recJ $O/recJ2 > $O/self-diff.txt 2>&1; tail -20 $O/self-diff.txt
(cd native/host && rm -f target/release/backplane-hub && systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- cargo build --release --bin backplane-hub > ../../$O/build.log 2>&1); bh=$?; grep -E '^error|Finished' $O/build.log | head -8 > $O/build.txt; [ $bh -eq 0 ] || exit $bh
cat $O/build.txt
HUB=${CARGO_TARGET_DIR:-$PWD/native/host/target}/release/backplane-hub; [ -x $HUB ] || exit 3
# one job for everything (a start is scarce): the crate's tests too, before the replay
(cd native/host && cargo test --bin backplane-hub commands:: 2>&1 | grep -E "^error|test result|FAILED|panicked" | head -20) > $O/test.txt 2>&1; cat $O/test.txt
PARITY_HUB_ARGS="--policy-bin $PWD/pstage/bp-policy-page --window-bin $PWD/pstage/window --version-string 0.1.0-dev --commit ${COMMIT:-0000000}" \
  systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- bun test/parity/replay.mjs --rec $O/recJ --bin $HUB --port 3828 --out $O/replay --all > $O/replay-diff.txt 2>&1
grep -E "differences|identical" $O/replay-diff.txt; tail -60 $O/replay-diff.txt
