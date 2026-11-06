#!/bin/sh
# BOAT ONLY (fixtures, stand-in agents): replay a staged recording against a staged hub binary. No build.
# Needs stage/rec (a recording dir), stage/hub (the binary); env HUBARGS (extra hub flags), ONLY (comma list of scenarios). Artifacts out/bin-replay/
set -u
O=out/bin-replay; mkdir -p $O
export PATH="$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
chmod +x stage/hub stage/bp-policy-page stage/window test/slim/check_agents.sh 2>/dev/null
export BACKPLANE_PARITY_ROOT=$PWD PARITY_WORK=/tmp/parity-work
PARITY_HUB_ARGS="$(printf %s "${HUBARGS:-}" | sed "s#@STAGE@#$PWD/stage#g")" systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 -- \
  bun test/parity/replay.mjs --rec $PWD/stage/rec --bin $PWD/stage/hub --port 3828 --out $O/replayA --all ${ONLY:+--only $ONLY} > $O/replay-diff.txt 2>&1 &
RP=$!
sleep 15
for p in $(pgrep -f "stage/hub --home"); do tr "\\0" " " < /proc/$p/cmdline > $O/hub-cmdline.txt; tr "\\0" "\\n" < /proc/$p/environ | grep -E "^(TZ|BACKPLANE_AGENTS)=" > $O/hub-env.txt; done
wait $RP
grep -E "differences|identical" $O/replay-diff.txt
