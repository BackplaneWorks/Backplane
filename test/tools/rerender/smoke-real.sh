#!/bin/sh
# The re-render harness on real threads, on demand, on this machine only:
# the hard rules (no loading row, no blank frame, nothing stuck, no agent
# started by the input itself, every window finishing) and the known
# failures named in the local budgets file, on a slice of the real log cut
# at a fixed point. Nothing real is in the repository: the threads, the cut
# and the known failures are in a local config, and the results go beside
# it.
#
#   test/tools/rerender/smoke-real.sh [BUILD]      BUILD defaults to build/rr
#
# The config (RR_REAL, default ~/Documents/i2cjak/bp-coord/rerender-harness/
# real-smoke.json): {"log": "~/.backplane-bend/events.jsonl", "long": "<id>",
# "tools": "<id>", "bg": "<id>", "until": <unix seconds>, "budgets":
# "<file: {fixture, known}>"}. Results: <config's folder>/smoke-<time>/.
# Heavy (a hub on a 30 MB slice, Chromium): run it under bp-admit
# (--mem 8 --cores 4), after `df -h /`; build first with build.sh.
set -eu
cd "$(dirname "$0")/../../.."
build=${1:-build/rr}
cfg=${RR_REAL:-$HOME/Documents/i2cjak/bp-coord/rerender-harness/real-smoke.json}
[ -f "$cfg" ] || { echo "smoke-real: no config at $cfg" >&2; exit 2; }
get() { python3 -c 'import json,os,sys; v=json.load(open(sys.argv[1])).get(sys.argv[2],""); print(os.path.expanduser(str(v)))' "$cfg" "$1"; }
[ "$(sh test/tools/rerender/stamp.sh)" = "$(cat "$build/backplane-serve.stamp" 2>/dev/null)" ] || { echo "smoke-real: $build is not this tree's build (test/tools/rerender/build.sh $build)" >&2; exit 3; }
out="$(dirname "$cfg")/smoke-$(date +%Y%m%d-%H%M%S)"
exec bun test/tools/rerender/rerender.ts --fixture real --log "$(get log)" \
  --long "$(get long)" --tools "$(get tools)" --bg "$(get bg)" --until "$(get until)" \
  --bin "$build/backplane-serve" --web "$build/web" --port "${RR_PORT:-3831}" \
  --scenarios open,idle,type1,burst,scroll,stress,clock,switch \
  --gate "$(get budgets)" --out "$out"
