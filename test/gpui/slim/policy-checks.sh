#!/bin/sh
# On Boat after build-split.sh: the policy's own checks against out/split/bp-policy.stripped (synthetic logs only).
set -u
export PATH="$HOME/.bun/bin:$PATH"
which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
B=$PWD/out/split/bp-policy.stripped; mkdir -p out/pl; bun test/gpui/slim/gen_log.mjs 12 12 > out/pl/log.txt
rc=0
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- bun test/gpui/slim/policy_local.mjs $B $PWD/out/pl/log.txt 3841 2>&1 | tee out/pl/policy_local.txt; grep -q "ALL OK" out/pl/policy_local.txt || rc=1
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- bun test/gpui/slim/policy_compact.mjs $B 3861 2 > out/pl/policy_compact.txt 2>&1 || rc=1; tail -3 out/pl/policy_compact.txt
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -- bun test/gpui/slim/policy_room.mjs $B 3871 2>&1 | tee out/pl/policy_room.txt; grep -q "ALL OK" out/pl/policy_room.txt || rc=1
exit $rc
