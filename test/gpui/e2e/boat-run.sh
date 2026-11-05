#!/bin/bash
# Desktop-side launcher: starts a Boat job (nothing runs here). Ships the hub and GPUI binaries as untracked files under test/gpui/e2e/bin
# (Boat never ships build/ or ignored files), removes them afterwards, never commits them.
#   test/gpui/e2e/boat-run.sh selftest                      unit tests of lib/frames.mjs + tool probe
#   test/gpui/e2e/boat-run.sh run HUB_BIN GPUI_BIN [journey]  GPUI journeys against that hub (Rust backplane-hub or JS backplane-serve)
set -u; cd "$(dirname "$0")/../../.." || exit 1
mode=${1:-selftest}; mkdir -p test/gpui/e2e/bin; trap 'rm -rf test/gpui/e2e/bin' EXIT
EXC=(--exclude 'mobile/**' --exclude 'desktop/**' --exclude 'docs/**' --exclude 'tools/**' --exclude 'native/**' --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' --exclude 'skills/**' --exclude 'extra/**')
case $mode in
 selftest) scripts/boat-job.sh --size default --timeout 900 --idle 0 --no-reuse --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh selftest ;;
 run) cp --reflink=auto "$2" test/gpui/e2e/bin/hub; cp --reflink=auto "$3" test/gpui/e2e/bin/gpui
      scripts/boat-job.sh --size default --timeout 3000 --idle 0 --no-reuse --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh run "${4:-all}" ;;
 *) echo "usage: $0 selftest | run HUB GPUI [journey]"; exit 2 ;;
esac
