#!/bin/sh
# desktop side. usage: REC=<dir> HUB=<binary> [HUBARGS="--version-string x"] [ONLY=join,badauth] sh test/gpui/parity/boat-bin-replay-launch.sh
set -eu
rm -rf stage; mkdir -p stage; cp -r "$REC" stage/rec; cp "$HUB" stage/hub
[ -f build/gpui/slim/bp-policy-page ] && cp build/gpui/slim/bp-policy-page stage/ || true
[ -f build/gpui/hub/new/window ] && cp build/gpui/hub/new/window stage/ || true
INC=$( { ls test/gpui/parity/*.mjs test/gpui/parity/*.sh test/gpui/wire.mjs test/gpui/perf/lib.mjs test/gpui/slim/check_agents.sh test/tools/perf_standin.ts src/web/wire.js src/web/view-delta.js src/core/cbor.bend; find stage -type f; } | sed 's/^/--include-file /' | tr '\n' ' ')
# shellcheck disable=SC2086
exec scripts/boat-job.sh --size large --timeout 900 --idle 3600 --artifact out/bin-replay $INC -- env HUBARGS="${HUBARGS:-}" ONLY="${ONLY:-}" sh test/gpui/parity/boat-bin-replay.sh
