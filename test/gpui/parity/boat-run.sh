#!/bin/sh
# Boat: run the parity harness under a hard cap, fixtures only (no store copy, no real conversations).
# Needs in the sandbox: bun, systemd-run --user, and the staged files (see boat-stage.sh). usage: boat-run.sh [rec|replay|all]
set -u
which bun >/dev/null 2>&1 || export PATH="$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
bun --version || exit 77
O=out/parity; mkdir -p $O
R=$PWD; export BACKPLANE_PARITY_ROOT=$R PARITY_WORK=/tmp/parity-work
chmod +x stage/backplane-serve test/gpui/slim/check_agents.sh 2>/dev/null
mode=${1:-all}
rc=0
if [ $mode = rec ] || [ $mode = all ]; then
  for t in A:3820 B:3822; do tag=${t%%:*}; port=${t##*:}; bun test/gpui/parity/record.mjs --bin $R/stage/backplane-serve --out $O/rec$tag --port $port --tag $tag > $O/rec$tag.log 2>&1; done
  bun test/gpui/parity/compare.mjs $O/recA $O/recB > $O/self-diff.txt 2>&1 || rc=1; cat $O/self-diff.txt
fi
if [ $mode = replay ] || [ $mode = all ]; then
  bun test/gpui/parity/replay.mjs --rec $O/recA --bin $R/stage/backplane-serve --port 3824 --out $O/replayA > $O/replay-diff.txt 2>&1 || rc=1; cat $O/replay-diff.txt
fi
exit $rc
