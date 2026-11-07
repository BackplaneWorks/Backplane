#!/bin/bash
# The Windows smoke test (CI's `windows smoke` job, in Git's bash on the runner): a hub on a temp home, the GPUI client connected to it with a
# script of one wait, then both end cleanly. It passes when the client drew a frame, reached "usable" (the hub's data drawn), said its GPU
# (WARP on a runner without one) and exited 0 by itself (--exit-after), and the hub then stopped on request.
#   HUB=<backplane-hub.exe> GPUI=<backplane-gpui.exe> test/windows/smoke.sh [PORT]
set -u
port=${1:-3811}; work=$(mktemp -d); home=$work/home; mkdir -p "$home"
log=$work/hub.log; clog=$work/client.log
"$HUB" --home "$home" --port "$port" --no-tailscale --foreground > "$log" 2>&1 & hub=$!
fail() { echo "SMOKE FAIL: $*"; echo "--- hub log"; tail -40 "$log"; echo "--- client log"; tail -60 "$clog" 2>/dev/null; kill $hub 2>/dev/null; exit 1; }
for i in $(seq 1 120); do curl -fsS "http://127.0.0.1:$port/hello" > /dev/null 2>&1 && break; kill -0 $hub 2>/dev/null || fail "the hub exited"; sleep 0.5; done
curl -fsS "http://127.0.0.1:$port/hello" > "$work/hello.json" || fail "no /hello"
echo "hub up: $(head -c 300 "$work/hello.json")"
[ -s "$home/token" ] || fail "no token in the home"
echo '[{"wait":1500}]' > "$work/steps.json"
cfg=$work/config; mkdir -p "$cfg"
XDG_CONFIG_HOME=$cfg BACKPLANE_GPUI_SCRIPT=$work/steps.json timeout 120 "$GPUI" --url "ws://127.0.0.1:$port" --token-file "$home/token" --exit-after 25 > "$clog" 2>&1; rc=$?
grep 'GPUI_METRIC' "$clog" | cut -c1-300
[ $rc = 0 ] || fail "the client exited $rc"
grep -q '"event":"first_frame"' "$clog" || fail "no first frame"
grep -q '"event":"usable"' "$clog" || fail "never usable (not connected, or the hub's data never drawn)"
grep -q '"event":"gpu"' "$clog" || fail "no GPU said"
kill $hub; for i in $(seq 1 20); do kill -0 $hub 2>/dev/null || break; sleep 0.5; done
kill -0 $hub 2>/dev/null && fail "the hub did not stop"
echo "SMOKE PASS"
rm -rf "$work"
