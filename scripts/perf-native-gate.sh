#!/bin/sh
# Supported fixed-host gate. No threshold bypass or diagnostic promotion.
set -eu
cd "$(dirname "$0")/.."
: "${BACKPLANE_PERF_FIXTURE:?set the immutable isolated benchmark log}"
: "${BACKPLANE_PERF_XVFB:?set the persistent test-only Xvfb path}"
[ -f "$BACKPLANE_PERF_FIXTURE" ] && [ -x "$BACKPLANE_PERF_XVFB" ] || exit 1
if [ "${1:-}" = --check-config ]; then
  bun scripts/perf-signature.ts --check-signing-config
  exit 0
fi
binary=${1:?usage: perf-native-gate.sh BINARY OUT}
out=${2:?usage: perf-native-gate.sh BINARY OUT}
receipt=$binary.perf-build.json
[ -s "$receipt" ] || { echo 'native performance build receipt missing' >&2; exit 1; }
[ -f "$BACKPLANE_PERF_FIXTURE" ] || exit 1
mkdir -p "$out"
(unset BACKPLANE_PERF_SIGNING_KEY; bun test/tools/perf_load.ts --bin "$binary" --log "$BACKPLANE_PERF_FIXTURE" \
  --xvfb "$BACKPLANE_PERF_XVFB" --display "${BACKPLANE_PERF_DISPLAY:-:178}" \
  --port "${BACKPLANE_PERF_PORT:-3899}" --exclusive-display --agents 10 --secs 20 \
  --phases bg,fg,type,open,scroll,plane,normal --keep --build-receipt "$receipt" \
  --evidence "$out/evidence.json" --out "$out/results.json")
bun scripts/perf-verify.ts "$out/evidence.json" "$binary" "$BACKPLANE_PERF_FIXTURE" \
  "$receipt" linux-native long-history timeline subagents rooms bots status > "$out/verified.json"
