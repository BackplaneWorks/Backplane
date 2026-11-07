#!/bin/sh
# desktop side: stage the JS hub binary and the presenter, launch the Boat job (work_run this). From the repo root (the settings worktree).
set -eu
ROOT=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
rm -rf stage; mkdir -p stage; cp $ROOT/build/gpui/backplane-serve-current stage/backplane-serve; cp $ROOT/build/gpui/slim/bp-policy-page $ROOT/build/gpui/hub/new/window stage/
{ git ls-files -co --exclude-standard native/host | grep -v '^native/host/target'
  ls test/parity/*.mjs test/parity/*.sh test/wire.mjs test/perf/lib.mjs test/slim/check_agents.sh test/settings/boat-parity.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor
  git ls-files -co --exclude-standard test/agents/fixtures test/files/hostile_paths.tsv
  find stage -type f; } | sed 's/^/--include-file /' | tr '\n' ' ' > stage/.args
# shellcheck disable=SC2046
scripts/boat-job.sh --size large --timeout 2400 --idle 3600 $(cat stage/.args) --artifact out/settings-parity -- sh test/settings/boat-parity.sh
