#!/bin/bash
# Fault injection: break a hub on a temp home in each way docs/HUB-RESILIENCE.md lists, and show it recovers with nothing lost.
#   test/guard/faults.sh BUILD_DIR [FAULT...]     BUILD_DIR holds backplane-hub and backplane; no FAULT runs them all
# Every hub is a real one on its own home and port, BACKPLANE_COPY=1, no tailscale, no agents. Fake builds are a shell line in front of the real hub, so what fails is the real code path.
# Each fault checks the log afterwards: every byte that was there before is still there. Env: FAULT_DIR (default the build's folder), FAULT_PORT (first port, default 3880).
set -u
BIN=$(cd "${1:?usage: faults.sh BUILD_DIR [FAULT...]}" && pwd); shift
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
T=$(mktemp -d "${FAULT_DIR:-$BIN}/.faults.XXXXXX"); PORT=$((${FAULT_PORT:-3880}-1)); PASS=0; FAILS=(); GP=
export BACKPLANE_COPY=1 BACKPLANE_AGENTS=$T/agents BACKPLANE_NO_UPDATE=1 BACKPLANE_GUARD_PROBE=1 BACKPLANE_GUARD_STABLE=2 BACKPLANE_GUARD_PAUSE=0 BACKPLANE_GUARD_MACHINE_PAUSE=1 BACKPLANE_GUARD_RESCUE_RETRY=3
export HOME=$T/userhome; mkdir -p "$T/agents" "$HOME"; unset BACKPLANE_AGENT_OWNER BACKPLANE_FAULT BACKPLANE_SAFE
G=$BIN/backplane
stopall() { if [ -n "$GP" ] && kill -0 "$GP" 2>/dev/null; then kill "$GP"; for _ in $(seq 60); do kill -0 "$GP" 2>/dev/null || break; sleep 0.25; done; kill -9 "$GP" 2>/dev/null; fi; GP=; }
trap 'stopall; [ -n "${FAULT_KEEP:-}" ] || rm -rf "${T:?}"' EXIT
say() { printf '  %s\n' "$*"; }
ok() { PASS=$((PASS+1)); printf 'PASS %-22s %s\n' "$CUR" "$*"; stopall; }
no() { FAILS+=("$CUR"); printf 'FAIL %-22s %s\n' "$CUR" "$*"; [ -z "${H:-}" ] || { tail -6 "$H/../hub.log" 2>/dev/null | cut -c1-160 | sed 's/^/    /'; }; stopall; }
home() { H=$T/$CUR/home; mkdir -p "$H"; PORT=$((PORT+1)); A=127.0.0.1:$PORT; python3 -I "$HERE/make-home.py" "$H" --small > /dev/null; cp "$H/events.jsonl" "$T/$CUR/before"; TOK=$(cat "$H/token"); }
# a build folder: the real programs, or a shell line in front of the real hub
build() { local d=$T/$CUR/builds/builds/$1; mkdir -p "$d"; rm -f "$d/backplane-hub"; ln -sf "$BIN/backplane" "$d/backplane"; if [ -n "${2:-}" ]; then printf '#!/bin/sh\n%s\n' "$2" > "$d/backplane-hub"; chmod +x "$d/backplane-hub"; else ln -sf "$BIN/backplane-hub" "$d/backplane-hub"; fi; echo "$1" > "$d/COMMIT"; }
# a build whose hub exits 101 at once. Never write into a build's hub: it may be a link to the real one
dead() { local f=$T/$CUR/builds/builds/$1/backplane-hub; rm -f "$f"; printf '#!/bin/sh\nexit 101\n' > "$f"; chmod +x "$f"; }
real() { printf 'BACKPLANE_FAULT=%s exec %s/backplane-hub "$@"' "$1" "$BIN"; }
guard() { # guard BUILD [ENV...]: run under a guard with the builds folder
  local b=$1; shift; rm -f "$H/guard/status.json"   # what the last guard said must not pass for what this one says
  env "$@" "$G" hub run --home "$H" --port "$PORT" --builds "$T/$CUR/builds" --build "$b" --log "$H/../hub.log" -- --no-tailscale --foreground >> "$H/../hub.log" 2>&1 < /dev/null & GP=$!
}
st() { python3 -I -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get(sys.argv[2],""))
except Exception: pass' "$H/guard/status.json" "$1"; }
hz() { curl -s -m3 "http://$A/healthz" | python3 -I -c 'import json,sys
try: print(json.loads(sys.stdin.read()).get(sys.argv[1],""))
except ValueError: pass' "$1"; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*5)) ]; do if "$@"; then return 0; fi; sleep 0.2; i=$((i+1)); done; return 1; }
is_up() { [ "$(hz status)" = ok ]; }
is_serving() { case "$(hz status)" in ok|degraded) return 0;; esac; return 1; }
on() { [ "$(st build)" = "$1" ] && [ "$(st stable)" = True ]; }
intact() { python3 -I - "$T/$CUR/before" "$H/events.jsonl" <<'PY'
import sys
a=open(sys.argv[1],'rb').read(); b=open(sys.argv[2],'rb').read()
sys.exit(0 if b.startswith(a) else 1)
PY
}
mkdir -p "$T/bin" && cp "$BIN/backplane" "$BIN/backplane-hub" "$BIN/COMMIT" "$T/bin/" && BIN=$T/bin && G=$BIN/backplane   # its own copies: a case cannot damage the build under test
want() { [ "${#WANT[@]}" = 0 ] || [[ " ${WANT[*]} " == *" $1 "* ]]; }
WANT=("$@")
go() { CUR=$1; want "$1" || return 1; mkdir -p "$T/$CUR/builds/builds"; home; return 0; }

if go corrupt-log; then
  { printf '%s\n' '{"$":"MessagePosted","thread":"t1","msg":"tornx'; head -c 300 /dev/urandom; printf '\n\0\0\0\n{"x":"\\é \\u0041"}\n'; } >> "$H/events.jsonl"; cp "$H/events.jsonl" "$T/$CUR/before"
  build good; guard good
  if ! until_ 30 is_up; then no "no start"; elif ! intact; then no "log changed"; elif [ ! -s "$H/quarantine/bad-lines.jsonl" ]; then no "bad lines not noted"; else ok "up on a corrupt log, $(wc -l < "$H/quarantine/bad-lines.jsonl") bad lines noted, log intact"; fi
fi
if go corrupt-checkpoints; then
  build good; guard good; until_ 30 is_up; sleep 12; stopall
  n=0; for f in "$H"/ckpt/*; do [ -f "$f" ] || continue; n=$((n+1)); python3 -I -c 'import sys,random
p=sys.argv[1]; b=bytearray(open(p,"rb").read()); r=random.Random(7)
for _ in range(40):
    if b: b[r.randrange(len(b))]^=0x55
open(p,"wb").write(b)' "$f"; done
  guard good
  if [ "$n" = 0 ]; then no "no checkpoints to corrupt"; elif ! until_ 30 is_up; then no "no start"; elif ! intact; then no "log changed"; else ok "$n checkpoint files corrupted, hub folded the log instead"; fi
fi
if go poison-record; then
  printf '%s\n' '{"$":"SettingSet","key":"poison.record","value":"x","at":9}' >> "$H/events.jsonl"; cp "$H/events.jsonl" "$T/$CUR/before"
  build poisoned "$(real panic-line:poison.record)"; guard poisoned
  if ! until_ 30 is_serving; then no "no start"; elif ! grep -q 'poison.record' "$H/quarantine/lines.jsonl" 2>/dev/null; then no "not quarantined"; elif ! intact; then no "log changed"; else ok "a record that panics a fold is quarantined; the log keeps it"; fi
fi
if go panicking-build; then
  build good; guard good; until_ 30 on good; stopall
  build bad "$(real die:store-now)"; dead bad
  echo bad > "$T/$CUR/builds/CURRENT"; guard bad
  if ! until_ 40 on good; then no "did not go back ($(st state) on $(st build))"; elif ! grep -q '^bad' "$T/$CUR/builds/BAD"; then no "not marked bad"; elif ! intact; then no "log changed"; else ok "3 failed starts: back on the last good build, marked bad: $(cut -f2 "$T/$CUR/builds/BAD" | cut -c1-50)"; fi
fi
if go watcher-will-not-retry; then
  build good; guard good; until_ 30 on good; stopall
  build bad; dead bad; printf 'bad\tpanicked\n' > "$T/$CUR/builds/BAD"; guard bad
  if ! until_ 20 on good; then no "started a bad build"; elif grep -q 'starting bad' "$H/../hub.log"; then no "tried it again"; else ok "a guard asked to start a build marked bad starts the good one"; fi
fi
if go part-panics; then
  build p "$(real panic:bots)"; guard p
  if ! until_ 30 is_serving; then no "no start"; elif [ "$(hz status)" != degraded ]; then no "says $(hz status)"; elif ! intact; then no "log changed"; else ok "bots panic at start: the hub serves without them and says so"; fi
fi
if go whole-process-dies; then
  # both builds take the process down in the same step: the home is at fault, not a build
  build good; guard good; until_ 30 on good; stopall
  build good "$(real die:agents)"; build next "$(real die:agents)"; guard next
  if ! until_ 60 eval '[ "$(st mode)" = safe ] && [ "$(hz status)" = degraded ]'; then no "no safe mode ($(st state) $(st mode))"; elif ! grep -q agents <<< "$(st skip)"; then no "the step that failed was not skipped ($(st skip))"; elif ! intact; then no "log changed"; else ok "every build dies at agents: safe mode on, agents skipped, hub serves"; fi
fi
if go safe-pauses-agents; then
  build good; guard good BACKPLANE_SAFE=0; until_ 30 is_up; "$G" hub safe --home "$H" --now > /dev/null
  if ! until_ 20 eval '[ "$(hz safe)" = True ]'; then no "not in safe mode"; elif ! grep -q 'SAFE MODE' "$H/../hub.log"; then no "no log line"; else
    "$G" hub normal --home "$H" --now > /dev/null; if until_ 20 eval '[ "$(hz safe)" = False ] && is_up'; then ok "safe, then normal, from one command each"; else no "did not come back"; fi; fi
fi
if go port-in-use; then
  build good; python3 -I -c 'import socket,sys,time
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(("127.0.0.1",int(sys.argv[1]))); s.listen(); time.sleep(int(sys.argv[2]))' "$PORT" 9 & HOLD=$!
  sleep 0.5; guard good BACKPLANE_GUARD_WINDOW=60
  until_ 8 eval '[ "$(st state)" = rescue ] || [ "$(st state)" = restarting ]' ; seen=$(st state); wait $HOLD
  if ! until_ 30 is_up; then no "did not recover when the port came free"; elif [ -s "$T/$CUR/builds/BAD" ]; then no "blamed the build"; else ok "port held: no build blamed ($seen), up once it came free"; fi
fi
if go hung-start; then
  build good; guard good; until_ 30 on good; stopall
  build stuck 'sleep 600'; guard stuck BACKPLANE_GUARD_STALL=2 BACKPLANE_GUARD_BOOT=3
  if ! until_ 60 on good; then no "did not go back ($(st state) on $(st build))"; elif ! grep -q stalled "$H/guard/incidents.jsonl"; then no "no incident"; else ok "a helper that hangs the start: killed, back on the good build"; fi
fi
if go hung-step; then
  build h "$(real hang:bots)"; guard h BACKPLANE_STEP_BUDGET=2
  if ! until_ 30 is_serving; then no "no start"; else ok "a step that hangs: the hub goes on without it after its budget"; fi
fi
if go hung-hub; then
  build good; guard good BACKPLANE_GUARD_MISSES=2; until_ 30 on good; P=$(st hubPid); kill -STOP "$P"
  if ! until_ 30 eval '[ "$(st hubPid)" != "$P" ] && [ "$(st hubPid)" != 0 ] && is_up'; then no "not restarted ($(st state))"; elif ! intact; then no "log changed"; else ok "a frozen hub is ended and started again"; fi
fi
if go full-disk; then
  # writes past the file size limit fail just as a full disk's do; SIGXFSZ is ignored so the failure arrives as an error
  build good; S=$(stat -c %s "$H/events.jsonl")
  ( trap '' XFSZ; ulimit -f "$((S/512))"; exec env "$G" hub run --home "$H" --port "$PORT" --builds "$T/$CUR/builds" --build good --log "$H/../hub.log" -- --no-tailscale --foreground >> "$H/../hub.log" 2>&1 < /dev/null ) & GP=$!
  if ! until_ 30 is_serving; then no "no start on a full disk"; else
    say "serving with writes failing: $(hz status)"; stopall; intact || no "log changed"
    guard good
    if until_ 30 is_up && intact; then ok "starts with writes failing, no torn log, whole once there is room"; else no "no recovery"; fi
  fi
fi
if go remote-restart; then
  DEV=0123456789abcdef0123456789abcdef; build good; guard good; until_ 30 on good
  c() { curl -s -m8 -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer $1" -d "{\"action\":\"$2\"}" "http://$A/hub/control"; }
  P=$(st hubPid)
  if [ "$(c nope restart)" != 401 ]; then no "a wrong token was let in"; elif [ "$(c "$DEV" stop)" = 202 ]; then no "stop from a device"; elif [ "$(c "$DEV" restart)" != 202 ]; then no "a paired device could not restart"; elif ! until_ 20 eval '[ "$(st hubPid)" != "$P" ] && is_up'; then no "did not come back"; else
    until_ 10 eval '[ "$(c "$DEV" restart)" = 202 ]'; sleep 3; until_ 20 is_up; c "$DEV" restart > /dev/null; sleep 3; until_ 20 is_up; r=$(c "$DEV" restart)
    if [ "$r" = 202 ]; then no "no limit on restarts"; else ok "device token restarts it, bad token refused, stop refused, 4th in 5 minutes refused ($r)"; fi
  fi
fi
if go rescue; then
  # nothing starts at all: the guard answers on the hub's own port, and a phone can bring it back
  DEV=0123456789abcdef0123456789abcdef; build dead; dead dead; guard dead BACKPLANE_GUARD_RESCUE_RETRY=30
  if ! until_ 40 eval '[ "$(hz status)" = down ]'; then no "no rescue ($(st state))"; else
    code=$(curl -s -m5 -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $DEV" "http://$A/hub/status")
    ln -sf "$BIN/backplane-hub" "$T/$CUR/builds/builds/dead/backplane-hub.new"; mv -f "$T/$CUR/builds/builds/dead/backplane-hub.new" "$T/$CUR/builds/builds/dead/backplane-hub"
    r=$(curl -s -m8 -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer $DEV" -d '{"action":"restart"}' "http://$A/hub/control")
    if [ "$code" != 200 ] || [ "$r" != 202 ]; then no "rescue refused the device ($code, $r)"; elif ! until_ 20 is_up; then no "did not come back"; elif ! intact; then no "log changed"; else ok "guard answers on the hub's port, a paired phone restarts it from there"; fi
  fi
fi
if go rollback-lockout; then
  DEV=0123456789abcdef0123456789abcdef; build old 'exec '"$BIN"'/backplane-hub "$@"'; build good; guard good; until_ 30 on good
  printf 'old\ngood\n' > "$H/guard/good"; printf 'good\n' > "$H/guard/control"
  r=$(curl -s -m8 -w '%{http_code}' -X POST -H "Authorization: Bearer $DEV" -d '{"action":"rollback"}' "http://$A/hub/control")
  case "$r" in *lock*409) "$G" hub rollback --home "$H" --now > /dev/null; if until_ 20 eval '[ "$(st build)" = old ]'; then ok "a remote rollback to a build with no remote control is refused; at the machine it works"; else no "local rollback failed"; fi;; *) no "allowed: $r";; esac
fi
if go update-probation; then
  build good; printf 'good\n' > "$T/$CUR/builds/CURRENT"; guard good; until_ 30 on good
  build next; dead next; "$G" hub update next --home "$H" --now > /dev/null
  if ! until_ 20 on good; then no "did not go back"; elif ! grep -q '^next' "$T/$CUR/builds/BAD"; then no "not marked bad"; elif [ "$(cat "$T/$CUR/builds/CURRENT")" != good ]; then no "CURRENT moved"; else
    build third; "$G" hub update third --home "$H" --now > /dev/null
    if until_ 20 on third && [ "$(cat "$T/$CUR/builds/CURRENT")" = third ]; then ok "an update that fails once goes straight back; one that holds moves CURRENT"; else no "the good update did not stick"; fi
  fi
fi
if go update-identity; then
  # an update proves it is the build it was staged as (EXPECT, from the signed manifest), not just a build that answers
  V=$("$BIN/backplane-hub" --version | awk '{print $2}'); C=$(cat "$BIN/COMMIT")
  build good; printf 'good\n' > "$T/$CUR/builds/CURRENT"; guard good; until_ 30 on good
  build next; printf '{"version":"%s","commit":"0000000000"}\n' "$V" > "$T/$CUR/builds/builds/next/EXPECT"; "$G" hub update next --home "$H" --now > /dev/null
  if ! until_ 30 on good; then no "did not go back ($(st state) on $(st build))"; elif ! grep -q '^next.*answered as' "$T/$CUR/builds/BAD"; then no "not marked bad for its identity: $(cat "$T/$CUR/builds/BAD")"; else
    build third; printf '{"version":"%s","commit":"%s"}\n' "$V" "$C" > "$T/$CUR/builds/builds/third/EXPECT"; "$G" hub update third --home "$H" --now > /dev/null
    if until_ 30 on third && [ "$(cat "$T/$CUR/builds/CURRENT")" = third ]; then ok "a build answering as another commit goes straight back, marked bad; the right one ($V/$C) sticks"; else no "the right build did not stick ($(st state) on $(st build))"; fi
  fi
fi
if go healthz-private; then
  # which build runs is told to a caller the hub knows, never to the open network
  build good; guard good; until_ 30 on good
  k() { curl -s -m3 "$@" "http://$A/healthz" | python3 -I -c 'import json,sys; v=json.loads(sys.stdin.read()); print(v.get("status"), "version" in v or "commit" in v)'; }
  none=$(k); badc=$(k -H "Authorization: Bearer nope"); tok=$(k -H "Authorization: Bearer $TOK")
  if [ "$none" != "ok False" ]; then no "no credential: $none"; elif [ "$badc" != "ok False" ]; then no "a bad credential: $badc"; elif [ "$tok" != "ok True" ]; then no "the hub token: $tok"; else ok "health for anyone; version and commit only with a credential"; fi
fi
if go full-temp; then
  # the temp folder nearly full while the home's disk is not: told once, and the hub and its agents get a temp folder on the home's disk. /dev/shm stands in for a full /tmp
  TD=/dev/shm/bp-fault-$$; mkdir -p "$TD"; tf=$(df -B1 --output=avail "$TD" | tail -1); hf=$(df -B1 --output=avail "$H" | tail -1)
  if [ "$hf" -le $((tf + (4 << 30))) ]; then say "skipped: /dev/shm ($tf) is not smaller than the home's disk ($hf)"; ok "skipped (no smaller filesystem to stand in for /tmp)"; else
    build good; guard good TMPDIR="$TD" BACKPLANE_GUARD_MIN_FREE=$(( (tf + hf) / 2 )); until_ 30 on good; P=$(st hubPid)
    if ! tr '\0' '\n' < "/proc/$P/environ" 2>/dev/null | grep -qx "TMPDIR=$H/tmp"; then no "the hub kept the full temp folder"; elif ! until_ 40 eval 'grep -q "Temp folder nearly full" "$H/guard/incidents.jsonl"'; then no "not told"; elif [ "$(grep -c 'Temp folder nearly full' "$H/guard/incidents.jsonl")" != 1 ]; then no "told more than once"; elif ! intact; then no "log changed"; else ok "a nearly full temp folder is told once; the hub and its agents write temp files on the home's disk"; fi
  fi
  rm -rf "$TD"
fi
if go cli-restarts-dead-guard; then
  build good; guard good; until_ 30 on good; kill -9 "$GP"; P=$(st hubPid); kill -9 "$P"; GP=; sleep 0.3
  "$G" hub restart --home "$H" --now > /dev/null
  if ! until_ 30 is_up; then no "nothing came back"; else GP=$(st guardPid); ok "guard and hub both killed: one command brings them back"; fi
fi
if go orphan-hub; then
  build good; guard good; until_ 30 on good; kill -9 "$GP"; GP=; sleep 0.2; guard good
  if until_ 30 eval 'on good && is_up'; then ok "a hub left by a killed guard is ended and replaced, not fought over the port"; else no "$(st state)"; fi
fi

echo; echo "$PASS passed, ${#FAILS[@]} failed${FAILS[*]:+: ${FAILS[*]}}"
[ "${#FAILS[@]}" = 0 ]
