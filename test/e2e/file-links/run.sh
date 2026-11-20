#!/bin/bash
# File links end to end (docs/file-links.md) on two EPHEMERAL Boat boxes. Run from a worktree on the desktop:
#   test/e2e/file-links/run.sh [HUB_BIN]
# The hub is built ON box A from this worktree's files (tracked and untracked, no .git; `cargo build --profile fast` there): a desktop build
# needs a newer glibc than the box image has. The binary is kept in $BP_TMP/fl-e2e-hub-<tree hash> and reused while the tree is the same.
# HUB_BIN: a hub already built for the box image (Ubuntu 24.04, glibc 2.39) instead.
#
# Box A runs this branch's hub on a temp home with a stand-in `claude` (test/tools/claude_standin.ts; no model turns) and a made-up project:
# mfg/big.bin (150 MB random), mech/bracket.step, .env, id_rsa, .git/config, a symlink to a file outside the project, and a file outside every
# project. Box B pairs as a device (POST /auth/invitations + /auth/enroll) and checks every route from the network (client.py). The two boxes
# reach each other over a WireGuard tunnel of their own (10.99.0.1 <-> 10.99.0.2, UDP 51900 opened only to the other box's address): Boat's
# own wg0 does not route between sandboxes. The hub listens only on 10.99.0.1:3799, --no-tailscale.
# Nothing of the person's goes to a box: the boxes start from the build snapshot (made with --no-env), get only the hub binary, the stand-in and
# these scripts. Both boxes are deleted at the end, also on failure or ^C (KEEP=1 keeps them and prints their ids).
# The desktop only runs boat, scp and ssh; the 150 MB file is made and moved between the boxes.
# Output: one line per check, a summary; the log is also in $BP_TMP/fl-e2e-<time>.log. Exit 1 when a check failed.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh" 2>/dev/null || BP_TMP=${TMPDIR:-$HOME/.cache}
files() { git -C "$ROOT" ls-files --cached --others --exclude-standard | grep -v -E '^(native/gpui/vendor|mobile)/'; }
TREE=$( { git -C "$ROOT" rev-parse HEAD; git -C "$ROOT" diff HEAD; files | grep -v '^test/e2e/file-links/' | while read -r f; do [ -f "$ROOT/$f" ] && git -C "$ROOT" ls-files --error-unmatch "$f" > /dev/null 2>&1 || cat "$ROOT/$f" 2>/dev/null; done; } | sha256sum | cut -c1-16)
BIN=${1:-$BP_TMP/fl-e2e-hub-$TREE}
TS=$(date +%s); LOG=$BP_TMP/fl-e2e-$TS.log; mkdir -p "$BP_TMP"
PORT=3799; AIP=10.99.0.1; BIP=10.99.0.2; WGP=51900
exec > >(tee -a "$LOG") 2>&1
b() { boat --no-update "$@"; }
on() { local id=$1; shift; b ssh "$id" -- "$*"; }
A=""; B=""
cleanup() {
  if [ "${KEEP:-0}" = 1 ]; then echo "KEEP=1: boxes kept: A=$A B=$B"; return; fi
  for id in $A $B; do b delete "$id" --yes > /dev/null 2>&1 && echo "deleted box $id" || echo "COULD NOT DELETE box $id: run boat delete $id --yes"; done
}
trap cleanup EXIT; trap 'exit 130' INT TERM
mk() { b new --from "${BP_BOAT_BASE:-bp-build-base}" --name "fl-e2e-$1-$TS" --environment bp-build --no-env --type "$2" --ttl 5400 --json 2>&1 | tail -1; }
field() { python3 -I -c 'import sys,json; print(json.loads(sys.stdin.read()).get(sys.argv[1]) or "")' "$1"; }
echo "== file-links E2E $(date -u +%FT%TZ), hub $(git -C "$ROOT" rev-parse --short HEAD) ($(git -C "$ROOT" branch --show-current)), tree $TREE"
ja=$(mk a large); A=$(echo "$ja" | field id); AV6=$(echo "$ja" | field ip)
jb=$(mk b default); B=$(echo "$jb" | field id); BV6=$(echo "$jb" | field ip)
[ -n "$A" ] && [ -n "$B" ] && [ -n "$AV6" ] && [ -n "$BV6" ] || { echo "FAIL could not create the boxes: ${ja:0:200} ${jb:0:200}"; exit 1; }
echo "boxes: A=$A B=$B"

# the tunnel: each side a key of its own, made on the box; only the public keys travel
wgkey() { on "$1" "sudo -n ufw allow from $2 to any port $WGP proto udp > /dev/null; sudo -n ip link add fl0 type wireguard 2>/dev/null; umask 077; wg genkey > /tmp/flwg; wg pubkey < /tmp/flwg" | tr -d '\r\n'; }
wgup() { on "$1" "sudo -n wg set fl0 listen-port $WGP private-key /tmp/flwg peer $2 allowed-ips $3/32 endpoint [$4]:$WGP persistent-keepalive 5 && sudo -n ip addr add $5/24 dev fl0 && sudo -n ip link set fl0 up && sudo -n ufw allow in on fl0 > /dev/null && rm -f /tmp/flwg"; }
APUB=$(wgkey "$A" "$BV6"); BPUB=$(wgkey "$B" "$AV6")
wgup "$A" "$BPUB" "$BIP" "$BV6" "$AIP"; wgup "$B" "$APUB" "$AIP" "$AV6" "$BIP"
if on "$B" "for i in \$(seq 20); do ping -c1 -W1 $AIP > /dev/null && exit 0; sleep 1; done; exit 1"; then echo "tunnel: B reaches A at $AIP"
else echo "FAIL tunnel between the boxes"; for id in $A $B; do on "$id" "sudo -n wg show fl0; ip -br addr show fl0"; done; exit 1; fi

on "$A" "mkdir -p ~/fl"; on "$B" "mkdir -p ~/fl"
if [ ! -x "$BIN" ]; then
  echo "building the hub on box A (no hub for tree $TREE yet)"; t0=$(date +%s)
  ( cd "$ROOT" && files | tr '\n' '\0' | tar --null -T - -czf - 2>/dev/null ) | b ssh "$A" -- 'mkdir -p ~/src && tar -xzf - -C ~/src' \
    || { echo "FAIL copying the sources to box A"; exit 1; }
  on "$A" '. ~/.cargo/env 2>/dev/null; cd ~/src/native/host && cargo build --profile fast --bin backplane-hub > ~/build.log 2>&1; rc=$?; tail -3 ~/build.log; exit $rc' \
    || { echo "FAIL the hub did not build on box A"; exit 1; }
  b scp "$A:/home/user/src/native/host/target/fast/backplane-hub" "$BIN.tmp" > /dev/null && mv "$BIN.tmp" "$BIN" && chmod +x "$BIN" || { echo "FAIL fetching the built hub"; exit 1; }
  echo "built in $(( $(date +%s) - t0 ))s, kept as $BIN"
fi
echo "hub binary sha256 $(sha256sum "$BIN" | cut -c1-12)"
b scp "$BIN" "$A:/home/user/fl/backplane-hub" > /dev/null && b scp "$ROOT/test/tools/claude_standin.ts" "$A:/home/user/fl/claude_standin.ts" > /dev/null \
  && b scp "$HERE/box_a.sh" "$A:/home/user/fl/box_a.sh" > /dev/null && b scp "$HERE/client.py" "$B:/home/user/fl/client.py" > /dev/null && b scp "$ROOT/native/host/src/wire_dict.rs" "$B:/home/user/fl/wire_dict.rs" > /dev/null || { echo "FAIL copying to the boxes"; exit 1; }

out=$(on "$A" "bash ~/fl/box_a.sh setup $AIP $PORT") || { echo "FAIL hub on box A did not start"; echo "$out" | grep -v '^token='; exit 1; }
kv() { echo "$out" | sed -n "s/^$1=//p" | tr -d '\r'; }
TOKEN=$(kv token); PROJ=$(kv proj); OUTSIDE=$(kv outside); HHOME=$(kv home)
SHA=$(on "$A" "bash ~/fl/box_a.sh sha" | sed -n 's/^sha=//p' | tr -d '\r'); SIZE=$((150 << 20))
echo "box A: hub up on $AIP:$PORT, project $PROJ, big.bin sha256 ${SHA:0:16}…"
args() { python3 -I -c 'import json,sys; print(json.dumps(dict(token=sys.argv[1], proj=sys.argv[2], outside=sys.argv[3], home=sys.argv[4], sha=sys.argv[5], size=int(sys.argv[6]))))' \
  "$TOKEN" "$PROJ" "$OUTSIDE" "$HHOME" "$SHA" "$SIZE" | base64 -w0; }
FAILS=0
phase() { on "$B" "python3 -I ~/fl/client.py $1 http://$AIP:$PORT \"\$(echo $(args) | base64 -d)\"" || FAILS=$((FAILS + 1)); }

on "$A" "find ~/src ~/fl -name .git -not -path '*/fl/proj/*' 2>/dev/null | head -3" | grep -q . && echo "FAIL a .git reached box A" || echo "pass  [hygiene] no .git on box A but the project's made-up one"
phase setup
# the network's own speed between the boxes, for the download times below (a plain http.server on A; not the hub)
on "$A" "cd ~/fl/proj/mfg && (setsid python3 -m http.server 3810 --bind $AIP > /dev/null 2>&1 < /dev/null & echo \$! > ~/fl/hs.pid); sleep 1" > /dev/null
on "$B" "curl -s -o /dev/null -m 120 -w 'info  [net] plain http.server A->B: %{size_download} B in %{time_total}s\n' http://$AIP:3810/big.bin"
on "$A" 'kill $(cat ~/fl/hs.pid)' > /dev/null
on "$A" "bash ~/fl/box_a.sh rss-start" > /dev/null
phase download
rss=$(on "$A" "bash ~/fl/box_a.sh rss-stop" | tr -d '\r'); echo "box A $rss"
base=$(echo "$rss" | sed -n 's/.*base=\([0-9]*\).*/\1/p'); peak=$(echo "$rss" | sed -n 's/.*peak=\([0-9]*\).*/\1/p')
if [ -n "$base" ] && [ -n "$peak" ] && [ $((peak - base)) -lt $((64 * 1024)) ]; then echo "pass  [rss] hub RSS during the 150 MB download: +$(((peak - base) / 1024)) MB (< 64 MB)"
else echo "FAIL  [rss] hub RSS during the 150 MB download: base=${base:-?} kB peak=${peak:-?} kB"; FAILS=$((FAILS + 1)); fi
phase checks
SHA=$(on "$A" "bash ~/fl/box_a.sh rewrite" | sed -n 's/^sha=//p' | tr -d '\r'); SIZE=$(((150 << 20) + 4096))
echo "box A: big.bin rewritten (new size and mtime), sha256 ${SHA:0:16}…"
phase stale
phase deny
phase shares
phase revoke
echo "-- box A hub log (warnings and errors):"; on "$A" "grep -iE 'error|panic|warn' ~/fl/hub.log | grep -v -i 'tailscale' | tail -15"
on "$A" "bash ~/fl/box_a.sh stop" > /dev/null
echo "== $( [ $FAILS = 0 ] && echo ALL PHASES PASSED || echo "$FAILS PHASE(S) FAILED") (log $LOG)"
[ $FAILS = 0 ]
