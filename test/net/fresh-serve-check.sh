#!/bin/bash
# fresh.sh and `tailscale serve`: a throwaway fresh hub touches only its own serve entry. RUN ON A MACHINE WITH NOTHING OF THE PERSON'S ON IT (h@backplane through bp-build),
# never on the desktop: it starts a hub and a window that cannot open (a dead DISPLAY), and edits this node's serve config.
#   BIN=<folder with backplane-hub and backplane-gpui> bash test/net/fresh-serve-check.sh
set -u
BIN=${BIN:?BIN=<folder with backplane-hub and backplane-gpui>}; HERE=$(cd "$(dirname "$0")/../.." && pwd); F=$HERE/scripts/fresh.sh
export DISPLAY=:99 FRESH_AGENT= BIN GPUI_DEV_DIR=/tmp/fresh-serve-dev
BY=8461; TH=8462; ok=1
say() { echo "[$1] $2"; [ "$1" = ok ] || ok=0; }
has() { tailscale serve status 2>/dev/null | grep -q "ts.net:$1 "; }
tailscale serve --bg --https=$BY http://127.0.0.1:3797 >/dev/null 2>&1; has $BY && say ok "bystander :$BY set" || { say FAIL "could not set the bystander"; exit 1; }
# 1. a throwaway asking for the bystander's port is refused, and the bystander stays
FRESH_DIR=/tmp/fresh-serve-a FRESH_PORT=3798 FRESH_TAILNET_PORT=$BY bash "$F" start > /tmp/fresh-serve-a.out 2>&1 && say FAIL "start on a taken port was not refused" || say ok "start on :$BY refused: $(tail -1 /tmp/fresh-serve-a.out | cut -c1-110)"
has $BY && say ok "bystander :$BY still there" || say FAIL "bystander lost"
# 2. a throwaway on its own port: starts, serves its own, stop removes only its own
FRESH_DIR=/tmp/fresh-serve-b FRESH_PORT=3798 FRESH_TAILNET_PORT=$TH bash "$F" start > /tmp/fresh-serve-b.out 2>&1; has $TH && say ok "throwaway serves :$TH" || say FAIL "throwaway has no :$TH (see /tmp/fresh-serve-b.out)"
FRESH_DIR=/tmp/fresh-serve-b FRESH_PORT=3798 FRESH_TAILNET_PORT=$TH bash "$F" stop 2>&1 | tail -2
has $TH && say FAIL "throwaway's :$TH left behind" || say ok "throwaway's :$TH removed"
has $BY && say ok "bystander :$BY untouched by start and stop" || say FAIL "bystander lost by the throwaway's stop"
# 3. someone took the throwaway's port over: its stop leaves that alone
FRESH_DIR=/tmp/fresh-serve-c FRESH_PORT=3799 FRESH_TAILNET_PORT=$TH bash "$F" start > /tmp/fresh-serve-c.out 2>&1
tailscale serve --bg --https=$TH http://127.0.0.1:3790 >/dev/null 2>&1   # another process now owns :$TH
FRESH_DIR=/tmp/fresh-serve-c FRESH_PORT=3799 FRESH_TAILNET_PORT=$TH bash "$F" stop 2>&1 | tail -2
has $TH && say ok "a port taken over after start is left alone by stop" || say FAIL "stop removed an entry that was not its own"
tailscale serve --https=$TH off >/dev/null 2>&1; tailscale serve --https=$BY off >/dev/null 2>&1
rm -rf /tmp/fresh-serve-a /tmp/fresh-serve-b /tmp/fresh-serve-c /tmp/fresh-serve-dev
[ $ok = 1 ] && echo "ALL OK" || { echo "FAILED"; exit 1; }
