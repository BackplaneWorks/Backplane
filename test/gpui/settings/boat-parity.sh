#!/bin/sh
# BOAT ONLY (fixtures, stand-in agents, no store copy): record the `settings` parity scenario on the JS hub (stage/backplane-serve), build the Rust hub
# from this tree, replay the recording against it. Artifacts out/settings-parity/{recJS,replay,replay-diff.txt,build.txt,planted.txt}
set -u
O=out/settings-parity; mkdir -p $O
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
chmod +x stage/backplane-serve stage/bp-policy-page stage/window test/gpui/slim/check_agents.sh 2>/dev/null
export BACKPLANE_PARITY_ROOT=$PWD PARITY_WORK=/tmp/parity-work
systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- bun test/gpui/parity/record.mjs --bin $PWD/stage/backplane-serve --out $O/recJS --port 3830 --only settings --tag S > $O/record.log 2>&1
tail -3 $O/record.log
# the planted value must be in no recorded frame, on either hub
grep -c "PARITY-planted" $O/recJS/settings.json > $O/planted.txt 2>&1; cat $O/planted.txt
(cd native/host && rm -f target/release/backplane-hub && systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 -- cargo build --release --bin backplane-hub > ../../$O/build.log 2>&1); bh=$?; grep -E '^error|Finished' $O/build.log | head -8 > $O/build.txt; [ $bh -eq 0 ] || exit $bh
cat $O/build.txt
HUB=$PWD/native/host/target/release/backplane-hub; [ -x $HUB ] || exit 3
PARITY_HUB_ARGS="--policy-bin $PWD/stage/bp-policy-page --window-bin $PWD/stage/window" \
  systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- bun test/gpui/parity/replay.mjs --rec $O/recJS --bin $HUB --port 3834 --out $O/replay --all > $O/replay-diff.txt 2>&1
grep -E "differences|identical" $O/replay-diff.txt
