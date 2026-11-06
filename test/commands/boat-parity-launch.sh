#!/bin/sh
# desktop side: stage the inputs and launch the Boat job (work_run this). usage from the repo root.
set -eu
rm -rf pstage; mkdir -p pstage; B=${GPUI_BUILD:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui}; cp $B/backplane-serve-960 pstage/backplane-serve; cp $B/slim/bp-policy-page $B/hub/new/window pstage/
{ git ls-files -co --exclude-standard native/host | grep -v '^native/host/target'
  ls test/parity/*.mjs test/parity/*.sh test/wire.mjs test/perf/lib.mjs test/slim/check_agents.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor
  git ls-files -co --exclude-standard test/agents/fixtures test/files/hostile_paths.tsv test/commands
  find pstage -type f; } | sed 's/^/--include-file /' | tr '\n' ' ' > pstage/.args
# shellcheck disable=SC2046
scripts/boat-job.sh --size large --timeout 2700 --idle 3600 $(cat pstage/.args) --artifact out/commands-parity -- sh test/commands/boat-parity.sh
