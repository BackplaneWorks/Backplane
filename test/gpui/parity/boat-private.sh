#!/bin/sh
# BOAT ONLY (fixtures only): the private-lines scenario on both hubs. Needs stage/js (the JS hub), stage/hub (the Rust hub), stage/bp-policy-page, stage/window.
# Records `private` twice from the JS hub (self-diff must be 0 and no frame may carry a sentinel), then replays it against the Rust hub.
set -u
O=out/private; mkdir -p $O
export PATH="$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
chmod +x stage/js stage/hub stage/bp-policy-page stage/window test/gpui/slim/check_agents.sh 2>/dev/null
export BACKPLANE_PARITY_ROOT=$PWD PARITY_WORK=/tmp/parity-work
rc=0
for t in A:3820 B:3822; do tag=${t%%:*}; port=${t##*:}
  bun test/gpui/parity/record.mjs --bin $PWD/stage/js --out $O/rec$tag --port $port --tag $tag --only private > $O/rec$tag.log 2>&1 || { echo "JS hub record $tag FAILED:"; tail -5 $O/rec$tag.log; rc=1; }
done
[ -s $O/recA/private.json ] && [ -s $O/recB/private.json ] || { echo 'NO private.json recorded (vacuous run)'; tail -5 $O/recA.log; exit 3; }
bun test/gpui/parity/compare.mjs $O/recA $O/recB | tail -3
PARITY_HUB_ARGS="--policy-bin $PWD/stage/bp-policy-page --window-bin $PWD/stage/window --version-string 0.1.0-dev" systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- \
  bun test/gpui/parity/replay.mjs --rec $O/recA --bin $PWD/stage/hub --port 3828 --out $O/replayA --all > $O/replay-diff.txt 2>&1 || rc=1
echo "-- Rust hub replay:"; cut -c1-240 $O/replay-diff.txt | head -30
exit $rc
