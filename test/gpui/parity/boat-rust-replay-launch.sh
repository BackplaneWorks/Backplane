#!/bin/sh
# desktop side: stage the inputs and launch the Boat job (work_run this; it prints the job's tail). usage from the repo root.
set -eu
rm -rf stage; mkdir -p stage; cp -r "${REC:-build/gpui/results/parity-current/recA}" stage/recA; cp build/gpui/slim/bp-policy-page build/gpui/hub/new/window stage/
{ git ls-files -co --exclude-standard native/host | grep -v '^native/host/target'
  ls test/gpui/parity/*.mjs test/gpui/parity/*.sh test/gpui/wire.mjs test/gpui/perf/lib.mjs test/gpui/slim/check_agents.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor.bend
  git ls-files -co --exclude-standard test/gpui/agents/fixtures test/gpui/files/hostile_paths.tsv
  find stage -type f; } | sed 's/^/--include-file /' | tr '\n' ' ' > stage/.args
# shellcheck disable=SC2046
scripts/boat-job.sh --size large --timeout 1800 $(cat stage/.args) --artifact out/rust-replay -- env COMMIT="${COMMIT:-}" VERSION="${VERSION:-}" sh test/gpui/parity/boat-rust-replay.sh
