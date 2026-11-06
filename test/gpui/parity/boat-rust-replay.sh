#!/bin/sh
# BOAT ONLY (fixtures, stand-in agents, no store copy): build the Rust hub from this tree and replay a recording against it with the presenter.
# Needs in the sandbox (see boat-rust-replay-launch.sh): the sources, test/gpui/parity, stage/recA, stage/bp-policy-page, stage/window.
# Artifacts: out/rust-replay/{replay-diff.txt,replayA,build.txt}
set -u
O=out/rust-replay; mkdir -p $O
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
(cd native/host && rm -f target/release/backplane-hub && systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- cargo build --release --bin backplane-hub > ../../$O/build.log 2>&1); bh=$?; grep -E '^error|Finished' $O/build.log | head -8 > $O/build.txt; [ $bh -eq 0 ] || exit $bh
cat $O/build.txt
HUB=$PWD/native/host/target/release/backplane-hub; [ -x $HUB ] || exit 3
chmod +x stage/bp-policy-page stage/window test/gpui/slim/check_agents.sh 2>/dev/null
export BACKPLANE_PARITY_ROOT=$PWD PARITY_WORK=/tmp/parity-work
PARITY_HUB_ARGS="--policy-bin $PWD/stage/bp-policy-page --window-bin $PWD/stage/window ${COMMIT:+--commit $COMMIT} ${VERSION:+--version-string $VERSION}" \
  systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- bun test/gpui/parity/replay.mjs --rec $PWD/stage/recA --bin $HUB --port 3828 --out $O/replayA --all > $O/replay-diff.txt 2>&1
grep -E "differences|identical" $O/replay-diff.txt
