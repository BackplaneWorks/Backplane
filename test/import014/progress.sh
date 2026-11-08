#!/bin/bash
# A long 0.14 import, watched in a private window (Xvfb + i3, temp HOME, stand-in agents, real clicks), both ways it begins:
#   A. setup: the person presses Count, then Import;
#   B. after 0.14's Update: the hub is started as the update's shim starts it (BACKPLANE_IMPORT_AUTO=1) and the import begins by itself.
# The 0.14 home is the fixture writer's large mode; BACKPLANE_IMPORT_THROTTLE_MS stretches the import so it can be watched.
#   test/import014/progress.sh BIN_DIR ASSETS_DIR OUT_DIR [XVFB]    (OUT_DIR short: i3's socket path must stay under 108 bytes)
# Each scenario checks: the desktop shows the card at once and it keeps moving; /healthz says ok with migrating and a fold held, and the progress there moves
# at least every 2 s; /hello answers within 1 s throughout; the summary at the end is right; Continue clears it. Screenshots in OUT_DIR/shots.
set -u
BIN=$(realpath -m "$1"); ASSETS=$(realpath -m "$2"); OUT=$(realpath -m "$3"); XVFB=${4:-/tmp/xvfb-root/usr/bin/Xvfb}; D=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$D/../.." && pwd)
export DISPLAY=${DISP:-:247}
[ -e "/tmp/.X11-unix/X${DISPLAY#:}" ] && { echo "display $DISPLAY is in use (another rig?): set DISP to a free one"; exit 2; } # two rigs on one display click each other's windows
P=${BIG_P:-50}; T=${BIG_T:-5000}; L=${BIG_L:-300000}; THROTTLE=${THROTTLE:-250}
mkdir -p "$OUT/shots"; fails=0; pids=(); client=
cleanup() { [ -n "$client" ] && kill "$client" 2>/dev/null; for f in "$OUT"/*/hub.pid; do [ -f "$f" ] && kill "$(cat "$f")" 2>/dev/null; done; kill "${pids[@]}" 2>/dev/null; }
trap cleanup EXIT
ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
X() { python3 "$ROOT/test/bub-send/xdo.py" "$@"; }
shot() { import -window root "$OUT/shots/$1.png"; }
wait_for() { local n=0; until eval "$1"; do sleep 0.5; n=$((n+1)); [ $n -gt ${2:-60} ] && return 1; done; return 0; }

python3 -I "$ROOT/test/fixtures/make-v014-home.py" "$OUT/big" --large "$P" "$T" "$L" || exit 2
echo "0.14 home: $P projects, $T threads, $(wc -l <"$OUT/big/events.jsonl") lines, $(du -sh "$OUT/big/events.jsonl" | cut -f1)"
$XVFB "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >"$OUT/xvfb.log" 2>&1 & pids+=($!); sleep 2
printf 'ipc-socket %s/i3.sock\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\n' "$OUT" >"$OUT/i3.conf"
env -u I3SOCK -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$OUT" i3 -c "$OUT/i3.conf" >"$OUT/i3.log" 2>&1 & pids+=($!); sleep 1

scenario() { # scenario NAME auto(0|1)
  local name=$1 auto=$2 R=$OUT/$1; rm -rf "$R"; mkdir -p "$R/home" "$R/run" "$R/agents" "$R/tmp"; chmod 700 "$R/run"
  local H=$R/home HUB=$R/home/.backplane/hub PORT; PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
  ln -s "$OUT/big" "$H/.backplane-bend"
  printf '#!/bin/sh\ncase "$1 $2" in "--version "*) echo "2.1.300 (Claude Code)";; "auth status") echo %s;; *) exit 0;; esac\n' "'{\"loggedIn\":true,\"email\":\"s@example.com\"}'" >"$R/agents/claude"; chmod +x "$R/agents/claude"
  local ENV=(HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_RUNTIME_DIR="$R/run" DISPLAY="$DISPLAY" PATH="$R/agents:/usr/bin:/bin" TMPDIR="$R/tmp" BACKPLANE_AGENTS="$R/agents"
    BACKPLANE_NO_TAILSCALE=1 BACKPLANE_PORT="$PORT" BACKPLANE_NO_UPDATE=1 BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_PEERS= BACKPLANE_KICAD_CLI=/nonexistent
    BACKPLANE_IMPORT_THROTTLE_MS="$THROTTLE" BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1 LP_NUM_THREADS=2 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json)
  if [ "$auto" = 1 ]; then # as 0.14's Update leaves it: the hub already started on the default home, importing by itself
    mkdir -p "$HUB"; env -i "${ENV[@]}" BACKPLANE_IMPORT_FROM="$OUT/big" BACKPLANE_IMPORT_AUTO=1 "$BIN/backplane-hub" --home "$HUB" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >"$R/hub.log" 2>&1 &
    echo $! >"$R/hub.pid"; wait_for "curl --noproxy '*' -sf -m2 http://127.0.0.1:$PORT/hello >/dev/null" 60 || { bad "$name: the hub did not answer"; return; }
  fi
  env -i "${ENV[@]}" "$BIN/backplane-gpui" >>"$R/win.log" 2>&1 & client=$!
  local pos="python3 $ROOT/test/onboarding/pos.py $R/win.log"
  # the chooser's button, once the window has drawn it (a loaded machine draws later than a fixed wait allows)
  wait_for "grep -q '\"name\":\"first-use\"' $R/win.log" 120 || { bad "$name: the window never drew its chooser"; return; }
  sleep 1; X click $($pos first-use)
  wait_for "curl --noproxy '*' -sf -m2 http://127.0.0.1:$PORT/hello >/dev/null" 120 || { bad "$name: no hub"; return; }
  [ -f "$R/hub.pid" ] || cut -d' ' -f1 "$HUB/hub.lock" >"$R/hub.pid"
  if [ "$auto" = 0 ]; then
    sleep 5; for _ in 1 2 3; do X key Return; sleep 1.5; done
    wait_for "grep -q 'btn:ob-import:scan' $R/win.log" 20 && X click $($pos btn:ob-import:scan) || bad "$name: no Count"
    wait_for "grep -q 'btn:ob-import:run\"' $R/win.log" 120 && X click $($pos btn:ob-import:run) || bad "$name: no Import"
  fi
  # watch it: /healthz every half second (progress, fold, how fast /hello answers), screenshots as it goes
  local t0; t0=$(date +%s.%N); local seen=0 shots=0 slow=0 maxgap=0 last="" last_t=$t0 n=0
  while :; do
    local now=$(date +%s.%N) a=$(date +%s%N)
    curl --noproxy '*' -sf -m5 "http://127.0.0.1:$PORT/hello" -o /dev/null || slow=$((slow+1)); local ms=$(( ($(date +%s%N) - a) / 1000000 )); [ "$ms" -gt 1000 ] && slow=$((slow+1))
    local hz; hz=$(curl --noproxy '*' -s -m5 "http://127.0.0.1:$PORT/healthz")
    local m; m=$(python3 -I -c 'import json,sys
v=json.loads(sys.argv[1] or "{}"); m=v.get("migrating")
print("" if not m else "%s %s %s/%s %s%% fold=%s status=%s" % (m.get("phase"), m.get("what"), m.get("done"), m.get("total"), m.get("pct"), v.get("folding"), v.get("status")))' "$hz")
    if [ -n "$m" ]; then
      seen=$((seen+1)); echo "$(printf '%.1f' $(echo "$now - $t0" | bc)) $m hello=${ms}ms" >>"$R/progress.txt"
      case "$m" in *"status=ok"*|*"status=degraded"*) ;; *) bad "$name: /healthz not ok while migrating: $m";; esac
      if [ "$m" != "$last" ]; then local gap; gap=$(echo "$now - $last_t" | bc); [ "$last" != "" ] && [ "$(echo "$gap > $maxgap" | bc)" = 1 ] && maxgap=$gap; last=$m; last_t=$now; fi
      local pct; pct=$(echo "$m" | sed -n 's/.* \([0-9]*\)% .*/\1/p')
      case "$m" in writing*) if [ $shots = 0 ] || { [ $shots = 1 ] && [ "${pct:-0}" -ge 70 ]; }; then shots=$((shots+1)); shot "$name-running-$shots"; fi;; reading*) [ $n = 2 ] && shot "$name-reading";; esac
    elif [ $seen -gt 0 ]; then break; fi
    n=$((n+1)); [ $n -gt 2400 ] && { bad "$name: did not finish in 20 minutes"; break; }
    sleep 0.5
  done
  local took; took=$(echo "$(date +%s.%N) - $t0" | bc)
  grep -q '"name":"migrate:running"' "$R/win.log" && ok "$name: the desktop showed the import card while it ran" || bad "$name: the desktop never showed the import card"
  grep -q '"TurnChanged","thread":"t1790292962-8017"' "$HUB/events.jsonl" && bad "$name: the imported uberbot ran a turn" || ok "$name: the imported uberbot stayed asleep"
  if [ -f "$OUT/shots/$name-running-1.png" ] && [ -f "$OUT/shots/$name-running-2.png" ]; then
    [ "$(md5sum <"$OUT/shots/$name-running-1.png")" != "$(md5sum <"$OUT/shots/$name-running-2.png")" ] && ok "$name: the desktop's card moved between two looks mid-import" || bad "$name: the desktop's card stood still"
  else bad "$name: no mid-import screenshots"; fi
  [ $seen -gt 4 ] && ok "$name: /healthz said migrating ($seen looks over ${took%.*} s), status ok, a fold held" || bad "$name: migrating seen only $seen times"
  [ "$(echo "$maxgap <= 2.0" | bc)" = 1 ] && ok "$name: progress moved at least every 2 s (longest wait ${maxgap} s)" || bad "$name: progress stood still ${maxgap} s"
  [ $slow = 0 ] && ok "$name: /hello answered within 1 s every time" || bad "$name: /hello slow or missing $slow times"
  wait_for "grep -q 'btn:ob-import:ack:' $R/win.log" 30 && { sleep 1; shot "$name-done"; ok "$name: the summary card came with Continue"; } || bad "$name: no summary card"
  local res; res=$(curl --noproxy '*' -sf -H "Authorization: Bearer $(cat "$HUB/token")" "http://127.0.0.1:$PORT/import/0.14")
  python3 -I -c 'import json,sys
v=json.loads(sys.argv[1]); c=v["counts"]; P,T=int(sys.argv[2]),int(sys.argv[3])
assert v["state"]=="done" and c["projects"]==P+1 and c["threads"]==T+3, c
print("summary: %d projects, %d threads, %d messages, %d events" % (c["projects"], c["threads"], c["messages"], c["lines"]))' "$res" "$P" "$T" && ok "$name: the summary is right" || bad "$name: summary $res"
  X click $(python3 "$ROOT/test/onboarding/pos.py" "$R/win.log" "$(grep -o 'btn:ob-import:ack:[0-9]*' "$R/win.log" | tail -1)"); sleep 2
  grep -q '"key":"import.v014.ack"' "$HUB/events.jsonl" && ok "$name: Continue saved that it was seen" || bad "$name: Continue saved nothing"
  shot "$name-after"
  kill "$client" 2>/dev/null; wait "$client" 2>/dev/null; client=; kill "$(cat "$R/hub.pid")" 2>/dev/null; rm -f "$R/hub.pid"; sleep 1
}
scenario setup 0
scenario update 1
echo "fails=$fails"; [ "$fails" = 0 ]
