#!/bin/bash
# Desktop-side launcher: starts a Boat job (nothing runs here). Ships the hub and GPUI binaries as untracked files under test/e2e/bin
# (Boat never ships build/ or ignored files), removes them afterwards, never commits them.
#   test/e2e/boat-run.sh selftest                      unit tests of lib/frames.mjs + tool probe
#   test/e2e/boat-run.sh run HUB_BIN GPUI_BIN [journey]  GPUI journeys against that hub (Rust backplane-hub or JS backplane-serve)
set -u; cd "$(dirname "$0")/../.." || exit 1
mode=${1:-selftest}; mkdir -p test/e2e/bin; trap 'rm -rf test/e2e/bin' EXIT
EXC=(--exclude 'mobile/**' --exclude 'desktop/**' --exclude 'docs/**' --exclude 'tools/**' --exclude 'native/**' --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' --exclude 'skills/**' --exclude 'extra/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'scripts/**' --exclude 'test/hub/**' --exclude 'test/slim/**' --exclude 'test/state/**' --exclude 'test/agents/**' --exclude 'test/net/**' --exclude 'test/parity/**' --exclude 'test/bend/**' --exclude 'test/term/**' --exclude 'test/screen/**' --exclude 'test/visual/**' --exclude 'test/wire/**' --exclude 'test/settings/**' --exclude 'test/web/**')
case $mode in
 selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh selftest ;;
 input-selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh input-selftest ;;
 input-run) cp --reflink=auto "$2" test/e2e/bin/hub; cp --reflink=auto "$3" test/e2e/bin/gpui; [ -d "$(dirname "$3")/assets" ] && cp -r "$(dirname "$3")/assets" test/e2e/bin/assets
      scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh input-run ;;
 run) cp --reflink=auto "$2" test/e2e/bin/hub; cp --reflink=auto "$3" test/e2e/bin/gpui; [ -d "$(dirname "$3")/assets" ] && cp -r "$(dirname "$3")/assets" test/e2e/bin/assets
      scripts/boat-job.sh --size default --timeout 3000 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh run "${4:-all}" ;;
 *) echo "usage: $0 selftest | run HUB GPUI [journey]"; exit 2 ;;
esac
