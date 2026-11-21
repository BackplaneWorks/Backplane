#!/bin/bash
# Desktop-side launcher: starts a Boat job (nothing runs here). Ships the hub and GPUI binaries as untracked files under test/e2e/bin
# (Boat never ships build/ or ignored files), removes them afterwards, never commits them.
#   (results: the job folder scripts/boat-job.sh names, artifacts/e2e)
#   test/e2e/boat-run.sh selftest                      unit tests of lib/frames.mjs + tool probe
#   test/e2e/boat-run.sh input-deadkey HUB_BIN GPUI_BIN  dead_acute then e through ibus over XIM gives é and the client lives (input/deadkey-run.mjs)
#   test/e2e/boat-run.sh run HUB_BIN GPUI_BIN [journey]  GPUI journeys against that hub (Rust backplane-hub or JS backplane-serve)
set -u; cd "$(dirname "$0")/../.." || exit 1
mode=${1:-selftest}; mkdir -p test/e2e/bin; trap 'rm -rf test/e2e/bin' EXIT
EXC=(--exclude 'mobile/**' --exclude 'desktop/**' --exclude 'docs/**' --exclude 'tools/**' --exclude 'native/**' --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' --exclude 'skills/**' --exclude 'extra/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'scripts/**' --exclude 'test/hub/**' --exclude 'test/slim/**' --exclude 'test/state/**' --exclude 'test/agents/**' --exclude 'test/net/**' --exclude 'test/parity/**' --exclude 'test/bend/**' --exclude 'test/term/**' --exclude 'test/screen/**' --exclude 'test/visual/**' --exclude 'test/wire/**' --exclude 'test/settings/**' --exclude 'test/web/**')
# the hub and the client, with the client's assets/ and the CEF runtime it loads from its own folder (scripts/cef-runtime-files.sh's linux list) (libcef.so and its data: symlinks in a dev build, copied as files)
ship() { [ -f "${1:-}" ] && [ -f "${2:-}" ] || { echo "usage: $0 $mode HUB_BIN GPUI_BIN"; exit 2; }; cp --reflink=auto "$1" test/e2e/bin/hub; cp --reflink=auto "$2" test/e2e/bin/gpui; d=$(dirname "$2")
  [ -d "$d/assets" ] && cp -rL "$d/assets" test/e2e/bin/assets
  for f in libcef.so icudtl.dat v8_context_snapshot.bin resources.pak chrome_100_percent.pak chrome_200_percent.pak locales libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json; do [ -e "$d/$f" ] && cp -rL "$d/$f" test/e2e/bin/; done; }
case $mode in
 selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh selftest ;;
 input-selftest) scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh input-selftest ;;
 input-run) ship "$2" "$3"
      scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh input-run ;;
 input-deadkey) ship "$2" "$3"
      scripts/boat-job.sh --size default --timeout 1200 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh input-deadkey ;;
 run) ship "$2" "$3"
      scripts/boat-job.sh --size default --timeout 3000 --idle 3600 --artifact out/e2e "${EXC[@]}" -- sh test/e2e/boat-inner.sh run "${4:-all}" ;;
 *) echo "usage: $0 selftest | input-selftest | input-run HUB GPUI | input-deadkey HUB GPUI | run HUB GPUI [journey]"; exit 2 ;;
esac
