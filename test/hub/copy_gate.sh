#!/bin/sh
# copy_gate.sh BIN : the binary refuses a home that looks like a copy of real hub data unless BACKPLANE_COPY=1, touching nothing.
# Synthetic logs only. Usage: sh test/hub/copy_gate.sh path/to/backplane-hub
BIN=${1:?binary}; D=$(mktemp -d /tmp/bp-copy-gate.XXXXXX); bad=0
ok()   { echo "ok   $1"; }
fail() { echo "FAIL $1"; bad=1; }
SYN='{"$":"ProjectCreated","id":"p1","title":"p","root":"/x","at":1}'
REAL='{"$":"SettingSet","key":"machine.id","value":"m1","at":3}'
# 1. a log with a real hub's machine.id, no switch: refused (exit 1), no lock/token/marker created, log untouched
mkdir -p $D/real; printf '%s\n%s\n' "$SYN" "$REAL" > $D/real/events.jsonl; sum=$(sha256sum $D/real/events.jsonl)
$BIN --home $D/real --port 3897 --no-tailscale --foreground > $D/real.out 2>&1; rc=$?
[ $rc = 1 ] && grep -q "BACKPLANE_COPY=1" $D/real.out && ok "real-looking log refused (exit $rc)" || fail "real-looking log not refused (exit $rc): $(cat $D/real.out)"
[ ! -e $D/real/hub.lock ] && [ ! -e $D/real/token ] && [ ! -e $D/real/hub.owner ] && ok "nothing created beside the log" || fail "files created: $(ls $D/real)"
[ "$sum" = "$(sha256sum $D/real/events.jsonl)" ] && ok "log unchanged" || fail "log changed"
# 2. a symlinked log: refused
mkdir -p $D/target $D/link; printf '%s\n' "$SYN" > $D/target/events.jsonl; ln -s $D/target/events.jsonl $D/link/events.jsonl
$BIN --home $D/link --port 3897 --no-tailscale --foreground > $D/link.out 2>&1; rc=$?
[ $rc = 1 ] && grep -q symlink $D/link.out && ok "symlinked log refused" || fail "symlink not refused (exit $rc)"
# 3. a synthetic log runs; a restart on its own home (now marked) runs too, even after it logged a machine.id
mkdir -p $D/syn; printf '%s\n' "$SYN" > $D/syn/events.jsonl
$BIN --home $D/syn --port 3897 --no-tailscale --foreground > $D/syn.out 2>&1 & P=$!
for i in $(seq 1 50); do curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && break; sleep 0.1; done
curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && ok "synthetic log runs" || fail "synthetic log does not run: $(cat $D/syn.out)"
kill $P; wait $P 2>/dev/null; [ -e $D/syn/hub.owner ] && ok "home marked" || fail "home not marked"
printf '%s\n' "$REAL" >> $D/syn/events.jsonl
$BIN --home $D/syn --port 3897 --no-tailscale --foreground > $D/syn2.out 2>&1 & P=$!
for i in $(seq 1 50); do curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && break; sleep 0.1; done
curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && ok "a marked home restarts without the switch" || fail "marked home refused: $(cat $D/syn2.out)"
kill $P; wait $P 2>/dev/null
# 3b. a plain truncated copy of a real log (no markers in it, over a megabyte) is refused too; a marked home of the same size is not
mkdir -p $D/trunc; python3 -c "
import json
l=json.dumps({'\$':'ActivityLogged','thread':'t1','act':'a','summary':'x'*80,'at':1})+'\n'
open('$D/trunc/events.jsonl','w').write(l*14000)"
$BIN --home $D/trunc --port 3897 --no-tailscale --foreground > $D/trunc.out 2>&1; rc=$?
[ $rc = 1 ] && grep -q "did not make the home" $D/trunc.out && ok "a plain truncated copy refused" || fail "truncated copy not refused (exit $rc): $(head -c 200 $D/trunc.out)"
# 4. with the switch the real-looking log runs, and says so
BACKPLANE_COPY=1 $BIN --home $D/real --port 3897 --no-tailscale --foreground > $D/real2.out 2>&1 & P=$!
for i in $(seq 1 50); do curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && break; sleep 0.1; done
curl -s -m 1 http://127.0.0.1:3897/hello > /dev/null && grep -q "BACKPLANE_COPY=1: a copy of a log" $D/real2.out && ok "BACKPLANE_COPY=1 runs it and says so" || fail "copy switch: $(cat $D/real2.out)"
kill $P; wait $P 2>/dev/null
rm -rf $D; echo "copy_gate: $([ $bad = 0 ] && echo PASS || echo FAIL)"; exit $bad
