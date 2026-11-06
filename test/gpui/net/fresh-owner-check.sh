#!/bin/bash
# fresh.sh and the hub's copy guard on a USED fresh home (a log over 1 MiB): a home without its hub.owner marker is refused by the hub alone, and fresh.sh start
# puts the marker back so the restart is not refused. RUN ON h@backplane through bp-build, never on the desktop.
#   BIN=<folder with backplane-hub and backplane-gpui> bash test/gpui/net/fresh-owner-check.sh
set -u
BIN=${BIN:?BIN=<folder with backplane-hub and backplane-gpui>}; HERE=$(cd "$(dirname "$0")/../../.." && pwd); F=$HERE/scripts/gpui/fresh.sh
export DISPLAY=:99 FRESH_AGENT= BIN GPUI_DEV_DIR=/tmp/fresh-owner-dev; ok=1
say() { echo "[$1] $2"; [ "$1" = ok ] || ok=0; }
mk() { rm -rf "$1"; mkdir -p "$1/home"; chmod 700 "$1" "$1/home"; touch "$1/.fresh"
       python3 -c "
import json
t=1700000000
with open('$1/home/events.jsonl','w') as f:
    f.write(json.dumps({'\$':'SettingSet','key':'agent.provider','value':'claude','at':t})+'\n')
    for i in range(6000): f.write(json.dumps({'\$':'MessagePosted','thread':'t1','msg':'m%d'%i,'role':'user','text':'x'*200,'at':t+i})+'\n')
"; }
A=/tmp/fresh-owner-a; mk $A; [ "$(stat -c %s $A/home/events.jsonl)" -gt 1048576 ] && say ok "used home: log $(stat -c %s $A/home/events.jsonl) bytes, no hub.owner" || say FAIL "log too small"
# the hub on its own refuses it
"$BIN/backplane-hub" --home $A/home --port 3795 --host 127.0.0.1 --no-tailscale --foreground > /tmp/fresh-owner-a.out 2>&1 &
HP=$!; sleep 2; kill $HP 2>/dev/null; wait $HP 2>/dev/null
grep -q "looks like a copy" /tmp/fresh-owner-a.out && say ok "the hub alone refuses the unmarked big log" || say FAIL "the hub did not refuse (guard changed?): $(head -c 200 /tmp/fresh-owner-a.out)"
# fresh.sh start puts the marker back and the hub starts
FRESH_DIR=$A FRESH_PORT=3795 bash "$F" start > /tmp/fresh-owner-a2.out 2>&1
[ -f $A/home/hub.owner ] && say ok "fresh.sh start wrote hub.owner" || say FAIL "no hub.owner after start"
curl -sf -m3 http://127.0.0.1:3795/hello > /dev/null && say ok "the hub started on the used home" || say FAIL "the hub did not start: $(tail -3 /tmp/fresh-owner-a2.out | cut -c1-160)"
FRESH_DIR=$A FRESH_PORT=3795 bash "$F" stop > /dev/null 2>&1
# a home fresh mode did not make is still refused by fresh.sh itself
B=/tmp/fresh-owner-b; mk $B; rm -f $B/.fresh
FRESH_DIR=$B FRESH_PORT=3794 bash "$F" start > /tmp/fresh-owner-b.out 2>&1 && say FAIL "start accepted a log fresh mode did not make" || say ok "a log fresh mode did not make is refused: $(tail -1 /tmp/fresh-owner-b.out | cut -c1-90)"
[ -f $B/home/hub.owner ] && say FAIL "a marker was written on a foreign home" || say ok "no marker written on the foreign home"
rm -rf $A $B /tmp/fresh-owner-dev /tmp/fresh-owner-*.out
[ $ok = 1 ] && echo "ALL OK" || { echo FAILED; exit 1; }
