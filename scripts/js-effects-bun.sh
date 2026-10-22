#!/bin/sh
# The server's JS effects on the loop the hub ships (Bun's event loop, the
# default): js_host_effects_test and js_pty_test emitted and run with bun.
# scripts/test.sh runs them in-process (`bend x.bend`), where they keep the
# runtime's sync loop (bend exits the process when its run returns), so this
# is the run that covers effects/loop.js. Each must exit 0, print ok lines
# and no FAIL: an exit 0 that ran nothing fails.
#
#   scripts/js-effects-bun.sh [OUTDIR]      (default build/js-effects)
set -u
cd "$(dirname "$0")/.."
out=${1:-build/js-effects}
mkdir -p "$out"
export BEND_NO_TELEMETRY=1
unset BACKPLANE_JS_LOOP
rc=0
for t in js_host_effects js_pty; do
  if ! bend "test/${t}_test.bend" -o "$out/$t.js" > "$out/$t.emit.log" 2>&1; then
    echo "js-effects-bun: $t did not emit" >&2
    tail -20 "$out/$t.emit.log" >&2
    rc=1
    continue
  fi
  code=0
  bun "$out/$t.js" > "$out/$t.out" 2>&1 || code=$?
  ok=$(grep -c '^ok' "$out/$t.out")
  fail=$(grep -c '^FAIL' "$out/$t.out")
  echo "js-effects-bun: $t: exit $code, $ok ok, $fail FAIL"
  if [ "$code" != 0 ] || [ "$ok" = 0 ] || [ "$fail" != 0 ]; then
    grep -E '^FAIL|deadlock|rror' "$out/$t.out" | head -20 >&2
    rc=1
  fi
done
exit $rc
