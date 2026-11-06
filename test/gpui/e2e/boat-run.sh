#!/bin/bash
# Desktop-side launcher: starts a Boat job (nothing runs here). Ships the hub and GPUI binaries as untracked files under test/gpui/e2e/bin
# (Boat never ships build/ or ignored files), removes them afterwards, never commits them.
#   test/gpui/e2e/boat-run.sh selftest                      unit tests of lib/frames.mjs + tool probe
#   test/gpui/e2e/boat-run.sh run HUB_BIN GPUI_BIN [journey]  GPUI journeys against that hub (Rust backplane-hub or JS backplane-serve)
set -u; cd "$(dirname "$0")/../../.." || exit 1
mode=${1:-selftest}; mkdir -p test/gpui/e2e/bin; trap 'rm -rf test/gpui/e2e/bin' EXIT
EXC=(--exclude 'mobile/**' --exclude 'desktop/**' --exclude 'docs/**' --exclude 'tools/**' --exclude 'native/**' --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' --exclude 'skills/**' --exclude 'extra/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'scripts/**' --exclude 'test/gpui/hub/**' --exclude 'test/gpui/slim/**' --exclude 'test/gpui/state/**' --exclude 'test/gpui/agents/**' --exclude 'test/gpui/net/**' --exclude 'test/gpui/parity/**' --exclude 'test/gpui/bend/**' --exclude 'test/gpui/term/**' --exclude 'test/gpui/screen/**' --exclude 'test/gpui/visual/**' --exclude 'test/gpui/wire/**' --exclude 'test/gpui/settings/**' --exclude 'test/gpui/web/**')
case $mode in
 selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh selftest ;;
 input-selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh input-selftest ;;
 input-run) cp --reflink=auto "$2" test/gpui/e2e/bin/hub; cp --reflink=auto "$3" test/gpui/e2e/bin/gpui; [ -d "$(dirname "$3")/assets" ] && cp -r "$(dirname "$3")/assets" test/gpui/e2e/bin/assets
      scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh input-run ;;
 run) cp --reflink=auto "$2" test/gpui/e2e/bin/hub; cp --reflink=auto "$3" test/gpui/e2e/bin/gpui; [ -d "$(dirname "$3")/assets" ] && cp -r "$(dirname "$3")/assets" test/gpui/e2e/bin/assets
      scripts/boat-job.sh --size default --timeout 3000 --idle 3600 --no-setup-bend --artifact out/e2e "${EXC[@]}" -- sh test/gpui/e2e/boat-inner.sh run "${4:-all}" ;;
 *) echo "usage: $0 selftest | run HUB GPUI [journey]"; exit 2 ;;
esac
