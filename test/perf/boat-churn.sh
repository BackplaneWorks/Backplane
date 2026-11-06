#!/bin/sh
# Hub churn on a seeded log, on Boat (the sandbox is the hard cap). Run from the repo root via scripts/boat-job.sh.
set -u
export NO_SCOPE=1 PERF_REPO=$PWD PERF_OUT=$PWD/out/churn HUB_BIN=$PWD/test/perf/bin/backplane-serve
mkdir -p out/churn
chmod +x test/perf/bin/backplane-serve
echo "sandbox: $(nproc) cpus; $(free -m | sed -n 2p); $(df -h . | tail -1)" | tee out/churn/env.txt
if command -v bun >/dev/null 2>&1; then RUN="bun"; echo "runner: bun $(bun --version)" | tee -a out/churn/env.txt
elif BUN_BE_BUN=1 ./test/perf/bin/backplane-serve --version >/dev/null 2>&1 && [ "$(BUN_BE_BUN=1 ./test/perf/bin/backplane-serve --version 2>&1 | head -1 | cut -c1-9)" != "backplane" ]; then RUN="env BUN_BE_BUN=1 $PWD/test/perf/bin/backplane-serve"; echo "runner: hub binary in bun mode" | tee -a out/churn/env.txt
else curl -fsSL https://bun.sh/install | bash >/dev/null 2>&1; export PATH=$HOME/.bun/bin:$PATH; RUN="bun"; echo "runner: installed bun $(bun --version)" | tee -a out/churn/env.txt; fi
cd test/perf
$RUN m4_churn.mjs ${1:-20} 3806 2>&1 | tail -5
cd ../..
python3 test/perf/fit.py out/churn/samples.jsonl 300 | tee out/churn/fit.txt
cat out/churn/done.json 2>/dev/null
