#!/bin/bash
# Can this build start? The last gate before dev-cut.sh publishes (scripts/dev-build.sh).
#   scripts/smoke-start.sh BUILD_DIR [PREVIOUS_BUILD_DIR]     BUILD_DIR holds backplane and backplane-hub
# On a throwaway real-shaped home (test/guard/make-home.py: every event the hub writes, right and wrong shapes, bad lines, a paired device) with BACKPLANE_COPY=1, no tailscale and an empty stand-in folder:
# the build before writes its checkpoints; this hub starts under this guard, serves, restarts, and restarts again from its own checkpoints; then the build before must start on what this one wrote.
# It fails on: no answer, a panic, a part of the hub not running, safe mode, a line quarantined, a log line lost. Env: SMOKE_DIR (where the home goes; default the system's temp folder), SMOKE_BOOT (s, default 90).
set -u
B=${1:?usage: smoke-start.sh BUILD_DIR [PREVIOUS_BUILD_DIR]}; P=${2:-}; HERE=$(cd "$(dirname "$0")" && pwd); BOOT=${SMOKE_BOOT:-90}
W=; GP=; HP=
die() { echo "error: smoke start: $*"; [ -z "$W" ] || tail -15 "$W/hub.log" 2>/dev/null; exit 1; }
[ -x "$B/backplane" ] && [ -x "$B/backplane-hub" ] || die "no backplane and backplane-hub in $B"
W=$(mktemp -d "${SMOKE_DIR:-${TMPDIR:-/tmp}}/.smoke.XXXXXX") || die "no temp folder"; H=$W/home
cleanup() {
  [ -z "$GP" ] || kill "$GP" 2>/dev/null; [ -z "$HP" ] || kill "$HP" 2>/dev/null
  for _ in 1 2 3 4 5 6 7 8 9 10; do { [ -n "$GP" ] && kill -0 "$GP" 2>/dev/null; } || { [ -n "$HP" ] && kill -0 "$HP" 2>/dev/null; } || break; sleep 0.3; done
  rm -rf "${W:?}"
}
trap cleanup EXIT
mkdir -p "$H" "$W/agents"
python3 -I "$HERE/../test/guard/make-home.py" "$H" > /dev/null || die "the home was not made"
PORT=$(python3 -I -c 'import socket
for p in range(3840,3990):
    s=socket.socket()
    try:
        s.bind(("127.0.0.1",p)); print(p); break
    except OSError: pass
    finally: s.close()'); [ -n "$PORT" ] || die "no free port"
export BACKPLANE_COPY=1 BACKPLANE_AGENTS=$W/agents BACKPLANE_NO_UPDATE=1 BACKPLANE_GUARD_PROBE=1 BACKPLANE_GUARD_STABLE=2 BACKPLANE_GUARD_TRIES=1
unset BACKPLANE_AGENT_OWNER BACKPLANE_SAFE BACKPLANE_FAULT
A=127.0.0.1:$PORT; T=$(cat "$H/token"); LINES0=$(wc -l < "$H/events.jsonl")
hz() { curl -s -m3 "http://$A/healthz"; }
field() { python3 -I -c 'import json,sys
try: v=json.loads(sys.stdin.read())
except ValueError: v={}
print(v.get(sys.argv[1],""))' "$1"; }
direct() { # a hub on its own, for the build before: started, asked, stopped
  "$1/backplane-hub" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$W/hub.log" 2>&1 < /dev/null & HP=$!
  local i=0 ok=1; while [ $i -lt $((BOOT*5)) ]; do if curl -sf -m2 -o /dev/null "http://$A/hello"; then ok=0; break; fi; kill -0 "$HP" 2>/dev/null || break; sleep 0.2; i=$((i+1)); done
  [ "$ok" != 0 ] || sleep "${2:-0}"; kill "$HP" 2>/dev/null; wait "$HP" 2>/dev/null; HP=; return $ok
}
if [ -n "$P" ] && [ -x "$P/backplane-hub" ]; then direct "$P" 12 || echo "warning: the build before did not start on this home (not this build's fault)"; fi
"$B/backplane" hub run --home "$H" --port "$PORT" --host 127.0.0.1 --bin "$B" --log "$W/hub.log" -- --no-tailscale --foreground >> "$W/hub.log" 2>&1 < /dev/null & GP=$!
well() {
  local i=0 s
  while [ $i -lt $((BOOT*5)) ]; do s=$(hz | field status); [ "$s" = ok ] && return 0; [ "$s" = degraded ] && die "$1: the hub runs without $(hz | field degraded)"; kill -0 "$GP" 2>/dev/null || die "$1: the guard left"; sleep 0.2; i=$((i+1)); done
  die "$1: no answer in $BOOT s ($(hz))"
}
well "first start"
curl -sf -m5 -o /dev/null "http://$A/hello" || die "/hello does not answer"
[ "$(hz | field safe)" = False ] || die "the hub came up in safe mode"
printf 'Authorization: Bearer %s\n' "$T" | curl -sf -m5 -H @- -o /dev/null "http://$A/hub/status" || die "/hub/status does not answer"
sleep 12   # the first checkpoints are written 10 s after a start
"$B/backplane" hub restart --home "$H" --now > /dev/null || die "restart failed"
well "start after a restart"
"$B/backplane" hub restart --home "$H" --now > /dev/null || die "second restart failed"
well "start from its own checkpoints"
[ "$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("mode"))' "$H/guard/status.json")" = normal ] || die "the guard left normal mode"
[ ! -s "$H/quarantine/lines.jsonl" ] || die "a fold broke on $(wc -l < "$H/quarantine/lines.jsonl") line(s): $(head -c 300 "$H/quarantine/lines.jsonl")"
[ ! -s "$H/guard/panic.txt" ] || die "$(head -1 "$H/guard/panic.txt")"
"$B/backplane" hub stop --home "$H" > /dev/null
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do kill -0 "$GP" 2>/dev/null || break; sleep 0.25; done; GP=
[ "$(wc -l < "$H/events.jsonl")" -ge "$LINES0" ] || die "log lines were lost"
if [ -n "$P" ] && [ -x "$P/backplane-hub" ]; then direct "$P" 0 || die "the build before cannot start on what this build wrote: a rollback would fail"; fi
echo "smoke start: ok ($(wc -l < "$H/events.jsonl") log lines)"
