#!/bin/sh
# desktop side: stage the inputs and launch the Boat job (work_run this). usage from the repo root.
set -eu
rm -rf pstage; mkdir -p pstage; B=${GPUI_BUILD:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui}; cp $B/backplane-serve-960 pstage/backplane-serve; cp $B/slim/bp-policy-page $B/hub/new/window pstage/
{ git ls-files -co --exclude-standard native/host | grep -v '^native/host/target'
  ls test/gpui/parity/*.mjs test/gpui/parity/*.sh test/gpui/wire.mjs test/gpui/perf/lib.mjs test/gpui/slim/check_agents.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor.bend
  git ls-files -co --exclude-standard test/gpui/agents/fixtures test/gpui/files/hostile_paths.tsv test/gpui/commands
  find pstage -type f; } | sed 's/^/--include-file /' | tr '\n' ' ' > pstage/.args
# shellcheck disable=SC2046
scripts/boat-job.sh --size large --timeout 2700 --idle 3600 $(cat pstage/.args) --artifact out/commands-parity -- sh test/gpui/commands/boat-parity.sh
