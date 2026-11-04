#!/bin/sh
set -eu
python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
export PATH="$HOME/.local/backplane-ci/bin:$PATH"
ulimit -s 1048576
bun install --cwd test/perf/bend-vs-ts --ignore-scripts
python3 test/perf/bend-vs-ts/run.py
