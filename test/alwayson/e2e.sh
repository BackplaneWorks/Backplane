#!/bin/bash
# The always-on Boat end to end, against a REAL Boat on the person's account (the desktop's `boat` CLI is signed in):
#   a throwaway hub home on this desktop (a free port in 3940-3999, file vault, no tailnet, no adoption of bp-hub), the Boat key put in its vault with
#   secret.set (from the CLI's own config, through a 0600 file, never an argument), then alwayson.act: public network, small, Make it.
#   Checks: every step done, the Boat linked and listed on Machines, the public door (healthz 200, /ws and /hub/status 401 to a stranger),
#   Delete it (two presses) and the Boat gone from `boat list`. A trap deletes the Boat whatever happens. bx_479dqxby is never touched.
#
#   test/alwayson/e2e.sh [HUB_BIN]    HUB_BIN: the backplane-hub to test (default native/host/target/debug/backplane-hub)
#   DICTATION=on also installs local dictation on the Boat (about 0.5 GB and a few minutes more). KEY_FILE: see below.
set -uo pipefail
export PATH="$HOME/.bun/bin:$HOME/.ascii/bin:$PATH"
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HUB_BIN=${1:-$ROOT/native/host/target/debug/backplane-hub}
DEV=${DEV:-$HOME/.backplane-gpui-dev}; CUR=$DEV/builds/$(head -1 "$DEV/CURRENT")
# the first free port in 3940-3999 (other lanes' throwaway hubs sit lower)
if [ -z "${PORT:-}" ]; then for p in $(seq 3940 3999); do ss -ltn 2> /dev/null | grep -q "127.0.0.1:$p " || { PORT=$p; break; }; done; fi
W=${W:-/tmp/bp-aon-e2e-$PORT}; H=$W/home; URL=ws://127.0.0.1:$PORT
say() { echo "aon-e2e: $*" >&2; }
fail() { say "FAIL: $*"; exit 1; }
rpc() { bun "$ROOT/scripts/hub-rpc.mjs" "$H/token" "$URL" "$@"; }
act() { rpc alwayson.act "{\"action\":\"$1\",\"value\":\"${2:-}\"}"; }
view() { rpc alwayson.view | python3 -I -c 'import sys,json; print(json.dumps(json.load(sys.stdin)["view"]))'; }
BOX=""
cleanup() {
  [ -n "$BOX" ] && echo "$BOX" >> /tmp/bp-aon-e2e-boats.txt
  [ -n "${HPID:-}" ] && kill "$HPID" 2> /dev/null
  # a Boat made before its id was seen here: the one named as this hub names it, made during this run
  [ -z "$BOX" ] && [ -n "${T0ISO:-}" ] && BOX=$(boat --no-update list --all --json 2> /dev/null | python3 -I -c 'import sys,json
t=sys.argv[1]; s=[x for x in json.load(sys.stdin)["sandboxes"] if x.get("name")=="Backplane always-on" and x.get("state") not in ("deleted","deleting") and (x.get("createdAt") or "")>=t and x["id"]!="bx_479dqxby"]
print(s[0]["id"] if s else "")' "$T0ISO")
  if [ -n "$BOX" ] && [ "$BOX" != bx_479dqxby ]; then
    if boat --no-update info "$BOX" --json > /dev/null 2>&1; then say "trap: deleting $BOX"; boat --no-update delete "$BOX" --yes > /dev/null 2>&1 || boat --no-update delete "$BOX" > /dev/null 2>&1 < /dev/null; fi
  fi
  rm -f "$W/k"
}
trap cleanup EXIT
[ -x "$HUB_BIN" ] || fail "no hub at $HUB_BIN"
rm -rf "$W"; mkdir -p "$H" "$W/builds"
# the build the Boat gets: this hub, and the guard, voice, assets beside it from the desktop's current build
B=$W/builds/aon-$(git -C "$ROOT" rev-parse --short HEAD); mkdir -p "$B"
cp "$HUB_BIN" "$B/backplane-hub"; for f in backplane backplane-voice silero_vad.onnx; do cp "$CUR/$f" "$B/"; done; cp -r "$CUR/assets" "$B/"
git -C "$ROOT" rev-parse HEAD > "$B/COMMIT"
say "throwaway hub on 127.0.0.1:$PORT (home $H, build $(basename "$B"))"
# SOURCE=release RELEASE=0.15.2: the Boat installs that published release (the path a user's hub takes) instead of this build
[ "${SOURCE:-}" = release ] && export BACKPLANE_ALWAYSON_SOURCE=release BACKPLANE_ALWAYSON_RELEASE="${RELEASE:-}"
# no BACKPLANE_URL and no tailnet: this hub is reachable on its own loopback only, as a user's with no tailnet is; it links one way and the Boat
# reaches it over its reverse link (HUB_URL=... sets an address instead)
env ${HUB_URL:+BACKPLANE_URL="$HUB_URL"} BACKPLANE_VAULT_KEYRING=file BACKPLANE_ALWAYSON_ADOPT=0 BACKPLANE_ALWAYSON_BUILD="$B" BACKPLANE_NO_UPDATE=1 BACKPLANE_NAME=aon-e2e \
  "$HUB_BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground --headless > "$W/hub.log" 2>&1 &
HPID=$!
for _ in $(seq 1 60); do curl -fsS -m 2 "http://127.0.0.1:$PORT/healthz" > /dev/null 2>&1 && break; sleep 1; done
curl -fsS -m 2 "http://127.0.0.1:$PORT/healthz" > /dev/null || fail "the hub did not start (tail $W/hub.log)"
sleep 3
# the person's Boat key into the throwaway vault, through a private file
# KEY_FILE: a key Boat's `api-key create --json` printed (its "secret"), e.g. one with only boat.rs ACTIONS; else the CLI's own sign-in
( umask 077; python3 -I -c 'import json,os,sys
f=sys.argv[1]
print(json.load(open(f))["secret"] if f else json.load(open(os.path.expanduser("~/.config/ascii/boat/config.json")))["token"])' "${KEY_FILE:-}" > "$W/k" )
bun "$ROOT/test/alwayson/put-secret.mjs" "$H/token" "$URL" boat.api_key "$W/k" || fail "secret.set"
rm -f "$W/k"
act alwayson-account > /dev/null || fail "the account was not read"
view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); a=v["account"]; print("account:", a["email"], a["plan"], "%.0f h left"%a["hours_left"], "missing:", a.get("missing")); print("sizes:", [s["line"] for s in v["sizes"]]); print("need:", v["need"])'
# EXPECT_MISSING=sandbox.create: the key is one that cannot make a Boat; the row must say so and Make it must not run
if [ -n "${EXPECT_MISSING:-}" ]; then
  view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); m=v["account"].get("missing") or []; assert sys.argv[1] in m, m; print("missing as expected:", m)' "$EXPECT_MISSING" || fail "the missing action was not seen"
  act alwayson-network public > /dev/null; out=$(act alwayson-make 2>&1) && fail "Make it ran with a key that cannot create: $out"
  say "PASS (Make it refused: $out)"; exit 0
fi
# NETWORK=tailnet TS_KEY_FILE=<a Tailscale auth key file>: the Boat joins the tailnet (the key goes into the throwaway vault as tailscale.boat)
if [ "${NETWORK:-public}" = tailnet ]; then
  [ -s "${TS_KEY_FILE:-}" ] || fail "NETWORK=tailnet needs TS_KEY_FILE"
  bun "$ROOT/test/alwayson/put-secret.mjs" "$H/token" "$URL" tailscale.boat "$TS_KEY_FILE" > /dev/null || fail "secret.set tailscale.boat"
fi
act alwayson-network "${NETWORK:-public}" > /dev/null; act alwayson-size small > /dev/null; act alwayson-dictation "${DICTATION:-off}" > /dev/null
T0ISO=$(date -u +%Y-%m-%dT%H:%M:%S)
act alwayson-make > /dev/null || fail "Make it was refused"
t0=$(date +%s); last=""
while :; do
  v=$(view); BOX=$(echo "$v" | python3 -I -c 'import sys,json; print((json.load(sys.stdin).get("boat") or {}).get("id",""))')
  line=$(echo "$v" | python3 -I -c 'import sys,json; v=json.load(sys.stdin); print(" ".join("%s:%s"%(s["id"],s["state"]) for s in v["steps"]), "running" if v["running"] else "")')
  [ "$line" != "$last" ] && { say "$(($(date +%s)-t0))s $line"; last=$line; }
  case "$line" in *fail*) echo "$v" | python3 -I -c 'import sys,json; [print("  failed:", s["label"], "-", s["note"]) for s in json.load(sys.stdin)["steps"] if s["state"]=="fail"]'; fail "a step failed";; esac
  echo "$line" | grep -q running || break
  [ $(($(date +%s)-t0)) -gt 1800 ] && fail "not done in 30 minutes"
  sleep 5
done
[ "$BOX" != bx_479dqxby ] || fail "it named bp-hub"
say "made $BOX in $(($(date +%s)-t0)) s"
PUB=$(view | python3 -I -c 'import sys,json; print(json.load(sys.stdin)["boat"]["url"])'); say "address $PUB"
# it appears on Machines (linked) and in the always-on row
rpc machines.page | python3 -I -c 'import sys,json
r=json.load(sys.stdin)["rows"]
lab=[x["label"] for x in r]; print("machines rows:", lab)
assert any(x["label"].startswith("Always-on:") for x in r), "no always-on row"
assert any("Paired" in x.get("note","") for x in r), "the Boat is not a paired machine here"' || fail "Machines does not show it"
# the public door, from outside
c() { curl -s -o /dev/null -w '%{http_code}' -m 20 "$@"; }
h=$(c "$PUB/healthz"); w=$(c -H 'Upgrade: websocket' -H 'Connection: Upgrade' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' -H 'Sec-WebSocket-Version: 13' "$PUB/ws?data=1")
s=$(c "$PUB/hub/status"); l=$(c -X POST -d '{}' "$PUB/bots/pair/local"); d=$(c "$PUB/client/capabilities")
say "door: healthz $h, /ws?data=1 $w, /hub/status $s, /bots/pair/local $l, /client/capabilities $d"
[ "$h" = 200 ] && [ "$w" = 401 ] && [ "$s" = 401 ] && [ "$l" = 401 ] && [ "$d" = 401 ] || fail "the public door let something through"
# Move your uberbot there: a fresh throwaway home has no uberbot (as at setup's step 4); it is made here, then moved to the Boat
LINK=$(view | python3 -I -c 'import sys,json; print(json.load(sys.stdin)["boat"].get("link",""))')
[ -n "$LINK" ] || fail "no link recorded for the Boat"
# A fresh Boat has no agent signed in, and the move's own check refuses until the uberbot's provider is signed in there (Sign it in there: a
# browser approval this script cannot give). MOVE=full expects the move to finish (sign the Boat in first); by default the press must make the
# uberbot here, get past "the home is not on this machine" to that check, and the row must say its reason.
if [ "${MOVE:-}" != full ]; then
  out=$(act alwayson-move "$LINK") && fail "the move started with nothing signed in on the Boat: $out"
  echo "$out" | grep -q "not signed in on" || fail "Move was refused for another reason: $out"
  view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); e=v.get("move_err",""); assert "not signed in on" in e, v; print("move row says:", e)' || fail "the row does not say why"
  grep -q '"name":"Bub"' "$H/events.jsonl" || fail "no uberbot was made here"
  say "move: the uberbot was made here, and the move's check said why it waits"
else
out=$(act alwayson-move "$LINK") || fail "Move was refused: $out"
for i in $(seq 1 120); do
  mv=$(view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); m=v.get("move") or {}; print(m.get("state",""), "|", m.get("step",""), "|", m.get("note",""), "|", v.get("move_err",""))')
  case "$mv" in done*) say "move: $mv"; break;; failed*) fail "the move failed: $mv";; esac
  [ $((i % 6)) = 0 ] && say "move: $mv"; sleep 5
done
case "$mv" in done*) ;; *) fail "the move did not end in 10 minutes: $mv";; esac
fi
# keep-alive and cost
act alwayson-check > /dev/null; view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); print("status:", v["status"], "cost:", v["cost"])'
# Delete it: two presses
act alwayson-delete | grep -q armed || fail "the first press did not arm Delete"
out=$(act alwayson-delete) || fail "Delete failed: $out"
sleep 5
st=$(boat --no-update info "$BOX" --json 2> /dev/null | python3 -I -c 'import sys,json; print(json.load(sys.stdin)["sandbox"]["state"])' 2> /dev/null || echo gone)
say "after Delete: $BOX is $st"
case "$st" in gone|deleted|deleting) ;; *) fail "the Boat is still $st";; esac
say "PASS ($BOX made, linked, door checked, Move ${MOVE:-checked}, deleted)"; echo "$BOX" >> "$W/../bp-aon-e2e-boats.txt"
BOX=""
