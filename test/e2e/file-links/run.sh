#!/bin/bash
# File links end to end (docs/file-links.md) on two EPHEMERAL Boat boxes. Run from a worktree on the desktop:
#   test/e2e/file-links/run.sh [HUB_BIN]
# The hub is built ON box A from this worktree's tracked files plus its untracked files under native/ (no .git, no scratch, nothing ignored;
# `cargo build --profile fast` there): a desktop build needs a newer glibc than the box image has. The binary is kept in
# $BP_TMP/fl-e2e-hub-<tree hash> and reused while the tree is the same. HUB_BIN: a hub already built for the box image (Ubuntu 24.04, glibc 2.39).
#
# Box A runs this branch's hub on a temp home with a stand-in `claude` (test/tools/claude_standin.ts; no model turns) and a made-up project:
# mfg/big.bin (150 MB random), mech/bracket.step, .env, id_rsa, .git/config, a symlink to a file outside the project, a file outside every
# project and one in <home>/far-files. Box B pairs as a device (POST /auth/invitations + /auth/enroll) and checks the routes from the network
# (client.py; its header lists what is and is not covered). The two boxes reach each other over a WireGuard tunnel of their own
# (10.99.0.1 <-> 10.99.0.2, UDP 51900 opened only to the other box's address): Boat's own wg0 does not route between sandboxes. The hub listens
# only on 10.99.0.1:3799, --no-tailscale.
# Nothing of the person's goes to a box: the boxes start from the build snapshot (made with --no-env), get only the sources above, the hub
# binary, the stand-in and these scripts. Both boxes are deleted at the end, also on failure or ^C (KEEP=1 keeps them and prints their ids).
# The desktop only runs boat, scp and ssh; the 150 MB file is made and moved between the boxes.
# Output: one line per check, a summary; the log is also in $BP_TMP/fl-e2e-<time>.log (share tokens masked). Exit 1 when a check failed.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh" 2>/dev/null || BP_TMP=${TMPDIR:-$HOME/.cache}
# the build's sources: tracked files, and untracked ones only under native/ (work in progress on the hub), never other scratch
untracked() { git -C "$ROOT" ls-files --others --exclude-standard -- native/ | grep -v -E '^native/gpui/vendor/'; }
files() { { git -C "$ROOT" ls-files --cached; untracked; } | grep -v -E '^(native/gpui/vendor|mobile)/'; }
TREE=$( { git -C "$ROOT" rev-parse HEAD; git -C "$ROOT" diff HEAD -- . ':!test/e2e/file-links'; untracked | while read -r f; do echo "$f"; cat "$ROOT/$f" 2>/dev/null; done; } | sha256sum | cut -c1-16)
BIN=${1:-$BP_TMP/fl-e2e-hub-$TREE}
TS=$(date +%s); LOG=$BP_TMP/fl-e2e-$TS.log; mkdir -p "$BP_TMP"
PORT=3799; AIP=10.99.0.1; BIP=10.99.0.2; WGP=51900
# tee -i: a ^C in the terminal must not take the log's pipe down before cleanup has deleted the boxes
exec > >(tee -i -a "$LOG") 2>&1
trap '' PIPE
b() { boat --no-update "$@"; }
on() { local id=$1; shift; b ssh "$id" -- "$*"; }
A=""; B=""
byname() { b list --all --json 2>/dev/null | python3 -I -c 'import sys,json
for x in json.load(sys.stdin)["sandboxes"]:
    if x.get("name") == sys.argv[1] and x.get("state") not in ("deleted", "deleting"): print(x["id"])' "$1"; }
cleanup() {
  local ids="$A $B" gone="" id
  # a box whose `boat new` was cut short has no id here: find it by its name
  [ -n "$A" ] || ids="$ids $(byname "fl-e2e-a-$TS")"; [ -n "$B" ] || ids="$ids $(byname "fl-e2e-b-$TS")"
  if [ "${KEEP:-0}" = 1 ]; then echo "KEEP=1: boxes kept:$ids"; return; fi
  for id in $ids; do b delete "$id" --yes > /dev/null 2>&1 && gone="$gone $id" || gone="$gone COULD-NOT-DELETE:$id"; done
  echo "deleted boxes:$gone"
}
trap cleanup EXIT; trap 'exit 130' INT TERM
mk() { b new --from "${BP_BOAT_BASE:-bp-build-base}" --name "fl-e2e-$1-$TS" --environment bp-build --no-env --type "$2" --ttl 5400 --json 2>/dev/null | grep '"id"' | tail -1; }
field() { python3 -I -c 'import sys,json
try: print(json.loads(sys.stdin.read()).get(sys.argv[1]) or "")
except ValueError: print("")' "$1"; }
mask() { sed -E 's#/s/[0-9a-f]{48}#/s/<token>#g; s#([?&]token=)[^& "]+#\1<token>#g'; }
echo "== file-links E2E $(date -u +%FT%TZ), hub $(git -C "$ROOT" rev-parse --short HEAD) ($(git -C "$ROOT" branch --show-current)), tree $TREE"
ja=$(mk a large); A=$(echo "$ja" | field id); AV6=$(echo "$ja" | field ip)
jb=$(mk b default); B=$(echo "$jb" | field id); BV6=$(echo "$jb" | field ip)
[ -n "$A" ] && [ -n "$B" ] && [ -n "$AV6" ] && [ -n "$BV6" ] || { echo "FAIL could not create the boxes: ${ja:0:200} ${jb:0:200}"; exit 1; }
echo "boxes: A=$A B=$B"

# the tunnel: each side a key of its own, made on the box; only the public keys travel
wgkey() { on "$1" "sudo -n ufw allow from $2 to any port $WGP proto udp > /dev/null; sudo -n ip link add fl0 type wireguard 2>/dev/null; umask 077; wg genkey > /tmp/flwg; wg pubkey < /tmp/flwg" | tr -d '\r\n'; }
wgup() { on "$1" "sudo -n wg set fl0 listen-port $WGP private-key /tmp/flwg peer $2 allowed-ips $3/32 endpoint '[$4]:$WGP' persistent-keepalive 5 && sudo -n ip addr add $5/24 dev fl0 && sudo -n ip link set fl0 up && sudo -n ufw allow in on fl0 > /dev/null && rm -f /tmp/flwg"; }
APUB=$(wgkey "$A" "$BV6"); BPUB=$(wgkey "$B" "$AV6")
wgup "$A" "$BPUB" "$BIP" "$BV6" "$AIP"; wgup "$B" "$APUB" "$AIP" "$AV6" "$BIP"
if on "$B" "for i in \$(seq 20); do ping -c1 -W1 $AIP > /dev/null && exit 0; sleep 1; done; exit 1"; then echo "tunnel: B reaches A at $AIP"
else echo "FAIL tunnel between the boxes"; for id in $A $B; do on "$id" "sudo -n wg show fl0; ip -br addr show fl0"; done; exit 1; fi

FAILS=0
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
# nothing but the made-up project's .git on either box (the sources went without theirs)
if dot=$(on "$A" "find ~/src ~/fl -name .git -not -path '*/fl/proj/*' 2>/dev/null | head -3; echo ok") && [ "$(echo "$dot" | tr -d '\r')" = ok ]; then echo "pass  [hygiene] no .git on box A but the project's made-up one"
else echo "FAIL  [hygiene] a .git on box A (or the look failed): $dot"; FAILS=$((FAILS + 1)); fi

out=$(on "$A" "bash ~/fl/box_a.sh setup $AIP $PORT") || { echo "FAIL hub on box A did not start"; echo "$out" | grep -v '^token='; exit 1; }
kv() { echo "$out" | sed -n "s/^$1=//p" | tr -d '\r'; }
TOKEN=$(kv token); PROJ=$(kv proj); OUTSIDE=$(kv outside); HHOME=$(kv home); FAR=$(kv far)
SHA=$(on "$A" "bash ~/fl/box_a.sh sha" | sed -n 's/^sha=//p' | tr -d '\r'); SIZE=$((150 << 20))
echo "box A: hub up on $AIP:$PORT, project $PROJ, big.bin sha256 ${SHA:0:16}…"
args() { python3 -I -c 'import json,sys; print(json.dumps(dict(token=sys.argv[1], proj=sys.argv[2], outside=sys.argv[3], home=sys.argv[4], sha=sys.argv[5], size=int(sys.argv[6]), far=sys.argv[7])))' \
  "$TOKEN" "$PROJ" "$OUTSIDE" "$HHOME" "$SHA" "$SIZE" "$FAR" | base64 -w0; }
phase() { on "$B" "python3 -I ~/fl/client.py $1 http://$AIP:$PORT \"\$(echo $(args) | base64 -d)\"" | mask; [ "${PIPESTATUS[0]}" = 0 ] || FAILS=$((FAILS + 1)); }

phase setup
# the network's own speed between the boxes, for the download times below (a plain http.server on A; not the hub)
on "$A" "cd ~/fl/proj/mfg && (setsid python3 -m http.server 3810 --bind $AIP > /dev/null 2>&1 < /dev/null & echo \$! > ~/fl/hs.pid); sleep 1" > /dev/null
on "$B" "curl -s -o /dev/null -m 120 -w 'info  [net] plain http.server A->B: %{size_download} B in %{time_total}s\n' http://$AIP:3810/big.bin"
on "$A" 'kill $(cat ~/fl/hs.pid)' > /dev/null
# the hub's RSS is sampled from before the first 150 MB download until the last one (/file, the resume, /files/get, the stale 200, the share)
on "$A" "bash ~/fl/box_a.sh rss-start" > /dev/null
phase download
phase checks
SHA=$(on "$A" "bash ~/fl/box_a.sh rewrite" | sed -n 's/^sha=//p' | tr -d '\r')
echo "box A: big.bin rewritten (same size, new contents and mtime), sha256 ${SHA:0:16}…"
phase stale
phase deny
phase shares
rss=$(on "$A" "bash ~/fl/box_a.sh rss-stop" | tr -d '\r'); echo "box A $rss"
base=$(echo "$rss" | sed -n 's/.*base=\([0-9]*\).*/\1/p'); peak=$(echo "$rss" | sed -n 's/.*peak=\([0-9]*\).*/\1/p'); n=$(echo "$rss" | sed -n 's/.*samples=\([0-9]*\).*/\1/p')
if [ -n "$base" ] && [ -n "$peak" ] && [ "${n:-0}" -gt 50 ] && [ $((peak - base)) -lt $((64 * 1024)) ]; then echo "pass  [rss] hub RSS over five 150 MB downloads: +$(((peak - base) / 1024)) MB (< 64 MB, $n samples)"
else echo "FAIL  [rss] hub RSS over the 150 MB downloads: base=${base:-?} kB peak=${peak:-?} kB samples=${n:-?}"; FAILS=$((FAILS + 1)); fi
phase revoke
echo "-- box A hub log (warnings and errors):"; on "$A" "grep -iE 'error|panic|warn' ~/fl/hub.log | grep -v -i 'tailscale' | tail -15" | mask
on "$A" "bash ~/fl/box_a.sh stop" > /dev/null
echo "== $( [ $FAILS = 0 ] && echo ALL PHASES PASSED || echo "$FAILS PHASE(S) FAILED") (log $LOG)"
[ $FAILS = 0 ]
