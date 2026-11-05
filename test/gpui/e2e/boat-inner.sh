#!/bin/sh
# Runs inside the Boat sandbox (repo root). The sandbox is the hard memory cap.
set -u; mkdir -p out/e2e; export NO_SCOPE=1 PERF_REPO=$PWD BOAT_SANDBOX=1
if command -v bun >/dev/null 2>&1; then RUN=bun; else curl -fsSL https://bun.sh/install | bash >/dev/null 2>&1; export PATH=$HOME/.bun/bin:$PATH; RUN=bun; fi
echo "runner: bun $($RUN --version); $(nproc) cpus; $(free -m | sed -n 2p)" | tee out/e2e/env.txt
case "$1" in
 selftest) $RUN test/gpui/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt
           $RUN -e "import('./test/gpui/e2e/lib/capture.mjs').then(m=>console.log(JSON.stringify(m.tools())))" | tee out/e2e/tools.txt ;;
 run) export E2E_SESSION_MIN=${E2E_SESSION_MIN:-3}; $RUN test/gpui/e2e/lib/frames.test.mjs 2>&1 | tee out/e2e/frames-test.txt | tail -12; chmod +x test/gpui/e2e/bin/*; $RUN test/gpui/e2e/run.mjs --client gpui --hub-bin test/gpui/e2e/bin/hub --gpui-bin test/gpui/e2e/bin/gpui --journey "${2:-all}" --record --out out/e2e 2>&1 | tail -40 ;;
esac
