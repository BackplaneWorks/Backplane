#!/bin/sh
# Boat only: build the Rust hub, then replay the post-#960 `unresolved` recording against it (fixtures only, stand-in agents).
# Output: out/unresolved/{replay.txt,expect.txt,self.json}. Needs bun and systemd-run --user in the sandbox (as test/parity/boat-run.sh).
exec scripts/boat-job.sh --size default --timeout 2400 --idle 3600 --artifact out/unresolved \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/hub/policy-bin/**' -- sh -c '
mkdir -p out/unresolved; (cd native/host && cargo build --release --bin backplane-hub > ../../out/unresolved/build.log 2>&1); ub=$?; tail -5 out/unresolved/build.log > out/unresolved/build.txt; [ $ub -eq 0 ] || exit $ub
which bun >/dev/null 2>&1 || export PATH="$HOME/.bun/bin:$PATH"; which bun >/dev/null 2>&1 || { curl -fsSL https://bun.sh/install | BUN_INSTALL=$HOME/.bun bash >/dev/null 2>&1; export PATH="$HOME/.bun/bin:$PATH"; }
R=$PWD; export BACKPLANE_PARITY_ROOT=$R PARITY_WORK=/tmp/pw; chmod +x test/slim/check_agents.sh
H=$R/native/host/target/release/backplane-hub
bun test/parity/record.mjs --bin $H --out out/unresolved/rec --port 3855 --tag rust --only unresolved > out/unresolved/record.txt 2>&1; r1=$?
bun test/parity/expect_unresolved.mjs out/unresolved/rec/unresolved.json > out/unresolved/expect.txt 2>&1; r2=$?
bun test/parity/replay.mjs --rec test/files/fixtures/unresolved-960 --bin $H --port 3856 --only unresolved --out out/unresolved/replay > out/unresolved/replay.txt 2>&1; r3=$?
cat out/unresolved/build.txt out/unresolved/expect.txt; grep -v logInfoKeys out/unresolved/replay.txt | tail -20; exit $(( ub | r1 | r2 | r3 ))'
