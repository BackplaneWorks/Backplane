#!/bin/sh
# Start each built binary (scripts/build.sh: the hub on Bun's runtime and
# its launcher) on a spare port with a throwaway home, run test/smoke.ts
# against it, stop it (by its PID). The test speaks CBOR through the hub's
# own codec, built here from test/wire. scripts/js-smoke.sh is the quick
# check of one binary.
set -eu
cd "$(dirname "$0")/.."
rm -rf build/wire
bend test/wire/index.html -o build/wire > /dev/null
run() {
  bin=$1 port=$2
  home=$(mktemp -d)
  mkdir "$home/agents"
  BACKPLANE_AGENTS="$home/agents" BACKPLANE_PEERS= \
    BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 "$bin" --foreground --no-tailscale --web dist/web --home "$home" --port "$port" > "$home/out.log" 2>&1 &
  pid=$!
  trap 'kill $pid 2>/dev/null || true' EXIT
  i=0
  until grep -q "listening" "$home/out.log" 2>/dev/null; do
    i=$((i + 1)); [ $i -lt 50 ] || { cat "$home/out.log"; exit 1; }
    sleep 0.1
  done
  bun test/smoke.ts "$port" "$home" build/wire
  kill $pid
  wait "$pid" 2>/dev/null || true
  trap - EXIT
  # a scratch thread's `git init` (the hub's child, not stopped with it)
  # may still be writing its folder: try again once it is done
  rm -rf "$home" 2>/dev/null || { sleep 1; rm -rf "$home"; }
}
run dist/backplane-serve 37931
[ -x dist/backplane ] && DISPLAY= run dist/backplane 37932
