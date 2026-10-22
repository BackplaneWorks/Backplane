#!/bin/sh
# The hub's quick smoke test: a backplane-serve that scripts/build-js.sh
# made, run on a throwaway home and a spare port, stopped by its PID.
#
#   scripts/js-smoke.sh BINARY [WEB_DIR]
#
# Checks --version, an unknown flag (exit 2), the hub answering /hello
# within BACKPLANE_SMOKE_SECS (default 20) and, with WEB_DIR, serving the
# web client's page. BACKPLANE_SMOKE_FULL=1 also runs test/smoke.ts (as
# scripts/smoke.sh does; it needs build/wire). POSIX sh, curl and
# nothing GNU: it runs on the Linux and macOS runners as is.
set -eu
bin=$1
web=${2:-}
port=${BACKPLANE_SMOKE_PORT:-$((38000 + $$ % 1000))}
secs=${BACKPLANE_SMOKE_SECS:-20}
cd "$(dirname "$0")/.."
fail() { echo "js-smoke: $*" >&2; exit 1; }
ver=$("$bin" --version) || fail "--version failed"
case $ver in "backplane "*) ;; *) fail "--version said '$ver'" ;; esac
rc=0; "$bin" --no-such-flag >/dev/null 2>&1 || rc=$?
[ "$rc" = 2 ] || fail "an unknown flag exited $rc, not 2"
home=$(mktemp -d "${TMPDIR:-/tmp}/bp-js-smoke.XXXXXX")
mkdir "$home/agents"
pid=""
stop() {
  # only the hub this script started, by the PID it captured
  if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  rm -rf "$home" 2>/dev/null || { sleep 1; rm -rf "$home" 2>/dev/null || true; }
}
trap stop EXIT
set -- --foreground --no-tailscale --home "$home" --port "$port"
[ -n "$web" ] && set -- "$@" --web "$web"
# stand-in agents (an empty folder: no real CLI runs) for the full test;
# the basic one starts no agent at all
agents=""
[ "${BACKPLANE_SMOKE_FULL:-}" = 1 ] && agents=$home/agents
BACKPLANE_TELEMETRY=off BACKPLANE_NO_UPDATE=1 BACKPLANE_GOOGLE_BAKE=0 \
  BACKPLANE_AGENTS=$agents BACKPLANE_PEERS= \
  "$bin" "$@" >"$home/out.log" 2>&1 &
pid=$!
i=0
until curl -sf --max-time 1 "http://127.0.0.1:$port/hello" >"$home/hello.json" 2>/dev/null; do
  kill -0 "$pid" 2>/dev/null || { cat "$home/out.log" >&2; pid=""; fail "the hub exited"; }
  i=$((i + 1)); [ "$i" -lt $((secs * 10)) ] || { cat "$home/out.log" >&2; fail "no /hello in ${secs} s"; }
  sleep 0.1
done
grep -aqF "${ver#backplane }" "$home/hello.json" || fail "/hello does not name ${ver#backplane }"
echo "js-smoke: $ver, /hello after about $((i / 10)).$((i % 10)) s"
if [ -n "$web" ]; then
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/") || code=000
  [ "$code" = 200 ] || fail "GET / answered $code"
  echo "js-smoke: the web client is served"
fi
if [ "${BACKPLANE_SMOKE_FULL:-}" = 1 ]; then
  bun test/smoke.ts "$port" "$home" build/wire
fi
echo "js-smoke: ok"
