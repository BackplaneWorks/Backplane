#!/bin/bash
# The 0.14 import on a first run, in a private window: Xvfb + i3, a temp HOME whose ~/.backplane-bend is a copy of test/fixtures/v014-home, stand-in
# agents, real clicks. Nothing here touches the person's screen, home or sign-ins.
#   test/import014/run.sh BIN_DIR ASSETS_DIR OUT_DIR [XVFB]      BIN_DIR holds backplane-hub and backplane-gpui side by side
# Prints one PASS/FAIL line per check, each tagged with its MATRIX row (I01..); screenshots in OUT_DIR/shots; exit 1 on any FAIL.
set -u
BIN=$(realpath -m "$1"); ASSETS=$(realpath -m "$2"); OUT=$(realpath -m "$3"); XVFB=${4:-/tmp/xvfb-root/usr/bin/Xvfb}; D=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$D/../.." && pwd)
export DISPLAY=${DISP:-:247}
PORT=${PORT:-$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')}
R=$OUT/rig; rm -rf "$R"; mkdir -p "$R/home" "$R/run" "$R/agents" "$R/tmp" "$OUT/shots"; chmod 700 "$R/run"
fails=0; pids=(); client=
cleanup() { [ -n "$client" ] && kill "$client" 2>/dev/null; [ -f "$R/hub.pid" ] && kill "$(cat "$R/hub.pid")" 2>/dev/null; kill "${pids[@]}" 2>/dev/null; }
trap cleanup EXIT
ok() { echo "PASS $1"; }; bad() { echo "FAIL $1"; fails=$((fails+1)); }
HOME_DIR=$R/home; HUB_HOME=$HOME_DIR/.backplane/hub; OLD=$HOME_DIR/.backplane-bend
case "$(realpath -m "$HOME_DIR")" in "$HOME"/.backplane*|"$HOME") echo "refused: not a temp home"; exit 2;; esac

# the 0.14 home: the fixture, as 0.14 left it in the user folder
cp -a "$ROOT/test/fixtures/v014-home" "$OLD"
sums() { (cd "$OLD" && find . -type f -print0 | sort -z | xargs -0 sha256sum; find . -type f -printf '%p %s %T@\n' | sort); }
sums >"$R/old-before.txt"
cat > "$R/agents/claude" <<'EOF'
#!/bin/sh
case "$1 $2" in "--version "*) echo "2.1.300 (Claude Code)";; "auth status") echo '{"loggedIn":true,"email":"standin@example.com","authMethod":"claude.ai"}';; *) exit 0;; esac
EOF
chmod +x "$R/agents/"*

$XVFB "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >"$R/xvfb.log" 2>&1 & pids+=($!); sleep 2
printf 'ipc-socket %s/i3.sock\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\n' "$R" >"$R/i3.conf"
env -u I3SOCK -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$R/run" i3 -c "$R/i3.conf" >"$R/i3.log" 2>&1 & pids+=($!); sleep 1
launch() {
  env -i HOME="$HOME_DIR" XDG_CONFIG_HOME="$HOME_DIR/.config" XDG_RUNTIME_DIR="$R/run" DISPLAY="$DISPLAY" PATH="$R/agents:/usr/bin:/bin" TMPDIR="$R/tmp" \
    BACKPLANE_AGENTS="$R/agents" BACKPLANE_NO_TAILSCALE=1 BACKPLANE_PORT="$PORT" BACKPLANE_NO_UPDATE=1 BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 BACKPLANE_PEERS= BACKPLANE_KICAD_CLI=/nonexistent \
    BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1 LP_NUM_THREADS=2 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json "$BIN/backplane-gpui" >>"$R/win.log" 2>&1 &
  client=$!
}
X() { python3 "$ROOT/test/bub-send/xdo.py" "$@"; }
shot() { import -window root "$OUT/shots/$1.png"; }
pos() { python3 "$ROOT/test/onboarding/pos.py" "$R/win.log" "$1"; }
wait_for() { local n=0; until eval "$1"; do sleep 0.5; n=$((n+1)); [ $n -gt ${2:-60} ] && return 1; done; return 0; }
hub_port() { local p; p=$(cut -d' ' -f2 "$HUB_HOME/hub.lock" 2>/dev/null); echo "${p:-$PORT}"; }
hub_up() { curl --noproxy "*" -sf -m 3 "http://127.0.0.1:$(hub_port)/hello" >/dev/null; }
api() { curl --noproxy "*" -sf -m 300 -H "Authorization: Bearer $(cat "$HUB_HOME/token")" "http://127.0.0.1:$(hub_port)$1"; }
count() { grep -c "$1" "$HUB_HOME/events.jsonl" 2>/dev/null || true; }
has_rect() { grep -q "\"name\":\"$1\"" "$R/win.log"; }

# 1. the chooser says 0.14's data is there, and starts the hub on a new home
launch; sleep 6; shot 00-chooser
X click $(pos first-use); wait_for hub_up 120 && ok "I00 the hub starts on the new home" || { bad "I00 no hub"; exit 1; }
cut -d' ' -f1 "$HUB_HOME/hub.lock" >"$R/hub.pid"
[ "$(count '"ThreadCreated"')" = 0 ] && ok "I00 nothing is imported before the person asks" || bad "I00 threads before any press"

# 2. setup: Welcome, Installed, Where your data is kept, then the import step
sleep 4; shot 01-welcome
for _ in 1 2 3; do X key Return; sleep 1.5; done
wait_for "has_rect btn:ob-import:scan" 20 && ok "I00 setup shows Bring your 0.14 projects across? with Count" || bad "I00 no import step"
shot 02-import-step
sleep 2; n0=$(wc -l <"$HUB_HOME/events.jsonl")
X click $(pos btn:ob-import:scan)
wait_for "has_rect btn:ob-import:run" 60 && ok "I00 Count shows what comes across and Import" || bad "I00 no Import after Count"
sleep 1; shot 03-counted
[ "$(wc -l <"$HUB_HOME/events.jsonl")" = "$n0" ] && ok "I00 counting wrote nothing" || { bad "I00 counting wrote to the log"; tail -n +$((n0 + 1)) "$HUB_HOME/events.jsonl" | cut -c1-160; }

# 3. Import (no keys)
X click $(pos btn:ob-import:run)
wait_for "grep -q 'import.v014.done' '$HUB_HOME/events.jsonl'" 120 && ok "I00 Import finished" || bad "I00 no import result"
sleep 2; shot 04-imported
[ "$(count '"ProjectCreated","id":"p1790175753-8"')" = 1 ] && [ "$(count '"ProjectCreated","id":"p1790175760-9"')" = 0 ] && ok "I01 projects: the live one, not the removed one" || bad "I01 projects"
[ "$(count '"ThreadCreated","id":"t1790175755-10"')" = 1 ] && [ "$(count '"ThreadCreated","id":"t1790175900-20"')" = 1 ] && [ "$(count '"msg":"m1790175790-18"')" = 1 ] && [ "$(count '"ActivityLogged"')" = 1 ] \
  && [ "$(count '"ThreadCreated","id":"run')" = 0 ] && [ "$(count '"ThreadCreated","id":"t1790175930-26"')" = 0 ] && ok "I02 threads with their history; not the mail run or the deleted thread" || bad "I02 threads"
[ "$(count '"TurnChanged"')" = 0 ] && [ "$(count '"AskOpened"')" = 0 ] && [ "$(count '"state":"interrupted"')" = 1 ] && ok "I02 threads arrive idle; the running task is interrupted" || bad "I02 turn state came across"
[ "$(count '"BotSet","id":"b1790292962-8017"')" = 1 ] && [ "$(count '"MemorySaved"')" = 1 ] && [ -f "$HOME_DIR/.backplane/bots/b1790292962-8017/notes.md" ] && ok "I03 the bot with its memory and folder" || bad "I03 bot"
[ "$(count '"RoutineSet"')" = 1 ] && [ "$(count '"on":true')" = 0 ] && ok "I04 the routine, switched off" || bad "I04 routine"
[ "$(count '"RoomSet"')" = 1 ] && [ "$(count '"RoomPosted"')" = 1 ] && ok "I05 the room with its post" || bad "I05 room"
[ "$(count '"key":"ui.theme","value":"dark"')" = 1 ] && [ "$(count 'm0123456789abcdef')" = 0 ] && [ "$(count 'told.k')" = 0 ] && ok "I06 settings that mean something; not 0.14's machine id or marks" || bad "I06 settings"
python3 - "$HUB_HOME/devices-v1.json" <<'EOF' && ok "I07 the live paired device joins the registry, the revoked one does not" || bad "I07 paired devices"
import json,sys; d=json.load(open(sys.argv[1])); ids=[x["id"] for x in d["devices"]]; assert "1"*32 in ids and "3"*32 not in ids, ids
EOF
[ "$(count "\"token\":\"$(printf 'a%.0s' $(seq 64))\"")" = 1 ] && ok "I07 the live push registration comes" || bad "I07 push registration"
S=$HUB_HOME/providers/accounts-v1/5e70d94b-6252-4f29-8eed-d3bd8c4b961a/02b33ae0-d452-432d-8838-bc364d86404d
[ -f "$S/projects/-home-a-Documents-Nordic/47119e0e-5017-4813-8353-70a01f309d22.jsonl" ] && [ ! -e "$S/.credentials.json" ] && grep -q 5e70d94b "$HUB_HOME/providers/accounts-v1/profiles.json" \
  && ok "I08 the account profile and its history; its login stays without keys" || bad "I08 account profile"
[ -f "$HUB_HOME/attachments/desktop_3787_t1790175755-10/datasheet.txt" ] && [ -f "$HUB_HOME/message-images/00a0.json" ] && ok "I09 attachments and message images" || bad "I09 attachments"
[ ! -e "$HUB_HOME/secrets/openai.key" ] && [ "$(count '"HookSet"')" = 0 ] && ok "I10 keys stay in 0.14 without consent" || bad "I10 keys came without consent"

# 4. with keys, from Settings' route: the vault's files and the login now come
api "/import/0.14/run?secrets=1" >"$R/run-keys.json"
python3 -c "import json,sys; v=json.load(open('$R/run-keys.json')); assert v['state']=='done' and v['secrets'], v" && [ -f "$HUB_HOME/secrets/openai.key" ] && [ -f "$S/.credentials.json" ] && [ "$(count '"HookSet"')" = 1 ] \
  && ok "I10 Import with keys brings the voice key, the webhook and the login" || bad "I10 import with keys"

# 5. again, and after a restart: nothing twice
n1=$(wc -l <"$HUB_HOME/events.jsonl")
api "/import/0.14/run" >"$R/run-again.json"
python3 -c "import json; v=json.load(open('$R/run-again.json')); assert v['counts']['lines']==0, v['counts']" && [ "$(tail -n +$((n1 + 1)) "$HUB_HOME/events.jsonl" | grep -vc '^{"\$":"SettingSet"')" = 0 ] && [ "$(tail -n +$((n1 + 1)) "$HUB_HOME/events.jsonl" | grep -c 'import.v014.done')" = 1 ] && ok "I11 a second import writes nothing but its result" || bad "I11 second import: $(head -c 300 "$R/run-again.json")"
kill "$client"; wait "$client" 2>/dev/null; client=; kill "$(cat "$R/hub.pid")"; wait_for "! hub_up" 30
n2=$(wc -l <"$HUB_HOME/events.jsonl")
launch; wait_for hub_up 120; cut -d' ' -f1 "$HUB_HOME/hub.lock" >"$R/hub.pid"; sleep 8; shot 05-second-start
[ "$(count '"ThreadCreated","id":"t1790175755-10"')" = 1 ] && [ "$(count '"MessagePosted"')" = 3 ] && ok "I11 a second start imports nothing again ($n2 lines before, $(wc -l <"$HUB_HOME/events.jsonl") after)" || bad "I11 the second start duplicated"
api "/import/0.14" | python3 -c "import json,sys; v=json.load(sys.stdin); assert v['state']=='done', v" && ok "I11 after the restart the hub says it was imported" || bad "I11 state after restart"

# 6. the 0.14 home is as it was
sums >"$R/old-after.txt"
cmp -s "$R/old-before.txt" "$R/old-after.txt" && ok "I12 the 0.14 home is unchanged: $(grep -c . "$R/old-before.txt") lines of hashes, sizes and times" || { bad "I12 the 0.14 home changed"; diff "$R/old-before.txt" "$R/old-after.txt" | head; }
echo "fails=$fails"; [ "$fails" = 0 ]
