#!/bin/bash
# The always-on Boat end to end, against a REAL Boat on the person's account (the desktop's `boat` CLI is signed in):
#   a throwaway hub home on this desktop (port 3911, file vault, no tailnet, no adoption of bp-hub), the Boat key put in its vault with
#   secret.set (from the CLI's own config, through a 0600 file, never an argument), then alwayson.act: public network, small, Make it.
#   Checks: every step done, the Boat linked and listed on Machines, the public door (healthz 200, /ws and /hub/status 401 to a stranger),
#   Delete it (two presses) and the Boat gone from `boat list`. A trap deletes the Boat whatever happens. bx_479dqxby is never touched.
#
#   test/alwayson/e2e.sh [HUB_BIN]    HUB_BIN: the backplane-hub to test (default native/host/target/debug/backplane-hub)
#   DICTATION=on also installs local dictation on the Boat (about 0.5 GB and a few minutes more).
set -uo pipefail
export PATH="$HOME/.bun/bin:$HOME/.ascii/bin:$PATH"
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HUB_BIN=${1:-$ROOT/native/host/target/debug/backplane-hub}
DEV=${DEV:-$HOME/.backplane-gpui-dev}; CUR=$DEV/builds/$(head -1 "$DEV/CURRENT")
PORT=${PORT:-3911}; W=${W:-/tmp/bp-aon-e2e}; H=$W/home; URL=ws://127.0.0.1:$PORT
say() { echo "aon-e2e: $*" >&2; }
fail() { say "FAIL: $*"; exit 1; }
rpc() { bun "$ROOT/scripts/hub-rpc.mjs" "$H/token" "$URL" "$@"; }
act() { rpc alwayson.act "{\"action\":\"$1\",\"value\":\"${2:-}\"}"; }
view() { rpc alwayson.view | python3 -I -c 'import sys,json; print(json.dumps(json.load(sys.stdin)["view"]))'; }
BOX=""
cleanup() {
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
IP=$(tailscale ip -4 2> /dev/null | head -1)
say "throwaway hub on 127.0.0.1:$PORT (home $H, build $(basename "$B"))"
BACKPLANE_URL="http://${IP:-192.0.2.1}:$PORT" BACKPLANE_VAULT_KEYRING=file BACKPLANE_ALWAYSON_ADOPT=0 BACKPLANE_ALWAYSON_BUILD="$B" BACKPLANE_NO_UPDATE=1 BACKPLANE_NAME=aon-e2e \
  "$HUB_BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground --headless > "$W/hub.log" 2>&1 &
HPID=$!
for _ in $(seq 1 60); do curl -fsS -m 2 "http://127.0.0.1:$PORT/healthz" > /dev/null 2>&1 && break; sleep 1; done
curl -fsS -m 2 "http://127.0.0.1:$PORT/healthz" > /dev/null || fail "the hub did not start (tail $W/hub.log)"
sleep 3
# the person's Boat key into the throwaway vault, through a private file
( umask 077; python3 -I -c 'import json,os; print(json.load(open(os.path.expanduser("~/.config/ascii/boat/config.json")))["token"])' > "$W/k" )
bun "$ROOT/test/alwayson/put-secret.mjs" "$H/token" "$URL" boat.api_key "$W/k" || fail "secret.set"
rm -f "$W/k"
act alwayson-account > /dev/null || fail "the account was not read"
view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); a=v["account"]; print("account:", a["email"], a["plan"], "%.0f h left"%a["hours_left"]); print("sizes:", [s["line"] for s in v["sizes"]]); print("need:", v["need"])'
act alwayson-network public > /dev/null; act alwayson-size small > /dev/null; act alwayson-dictation "${DICTATION:-off}" > /dev/null
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
# keep-alive and cost
act alwayson-check > /dev/null; view | python3 -I -c 'import sys,json; v=json.load(sys.stdin); print("status:", v["status"], "cost:", v["cost"])'
# Delete it: two presses
act alwayson-delete | grep -q armed || fail "the first press did not arm Delete"
out=$(act alwayson-delete) || fail "Delete failed: $out"
sleep 5
st=$(boat --no-update info "$BOX" --json 2> /dev/null | python3 -I -c 'import sys,json; print(json.load(sys.stdin)["sandbox"]["state"])' 2> /dev/null || echo gone)
say "after Delete: $BOX is $st"
case "$st" in gone|deleted|deleting) ;; *) fail "the Boat is still $st";; esac
say "PASS ($BOX made, linked, door checked, deleted)"; echo "$BOX" >> "$W/../bp-aon-e2e-boats.txt"
BOX=""
