#!/bin/bash
# The release-candidate smoke, on one machine of one target (release.yml's `smoke` job runs it on each target's own runner; it runs here too):
#   test/update/rc-smoke.sh REL
# REL: a release dry run's artifact (release-dry-run). From it: the archive installed as a person would (install.sh on Linux and macOS, the zip's
# `backplane-gpui.exe --install DIR` on Windows), the guard started on a private home and port, /hello naming the manifest's version and commit;
# the helpers there and answering (voice, step2glb on a real STEP, the browser helper); the installed client started against the hub with the
# Browser tab open on a local page: it draws (Linux: the embedded Chromium paints the page; macOS and Windows: no CEF, the helper's frames),
# and no asset is missing ("icon X not in icons.json"). Update and rollback are test/update/install-e2e.sh's (release.yml's smoke job runs both).
# Private: HOME, the install folder, the home, ports. Exit 0 when every check passes; PASS/FAIL lines on stdout.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
REL=$(cd "$1" && pwd)
PASS=0; FAILS=()
ok() { PASS=$((PASS+1)); printf 'PASS %-10s %s\n' "$1" "$2"; }
no() { FAILS+=("$1"); printf 'FAIL %-10s %s\n' "$1" "$2"; }
PY=$(command -v python3 || command -v python); py() { "$PY" -I "$@"; }   # Windows runners name it python
mf() { py -c 'import json,sys;print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$REL/manifest.json" "$1"; }
TAG=$(mf tag); VER=$(mf version); COMMIT=$(mf commit)
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) TGT=linux-x64;; Linux-aarch64) TGT=linux-arm64;; Darwin-arm64) TGT=darwin-arm64;; MINGW*|MSYS*|CYGWIN*) TGT=windows-x64;; *) echo "no target"; exit 2;;
esac
win=; [ "$TGT" = windows-x64 ] && win=1
X=; [ -n "$win" ] && X=.exe
fp() { py -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
T=$(mktemp -d "${RC_DIR:-${RUNNER_TEMP:-$ROOT/build}}/rc.XXXXXX"); P=$(fp); FP=$(fp); PP=$(fp)
H=$T/home; PREFIX=$T/prefix; mkdir -p "$H" "$T/u" "$T/feed" "$T/proj"
export HOME=$T/u BACKPLANE_AGENTS=$T/agents BACKPLANE_UPDATE_FEED=http://127.0.0.1:$FP/releases.json BACKPLANE_GUARD_STABLE=10
[ -z "$win" ] || export USERPROFILE="$(cygpath -w "$T/u")" LOCALAPPDATA="$(cygpath -w "$T/u")"
unset BACKPLANE_BUILDS BACKPLANE_GUARD BACKPLANE_NO_UPDATE BACKPLANE_COPY BACKPLANE_HOME
mkdir -p "$BACKPLANE_AGENTS"
G=$PREFIX/backplane$X
PIDS=; trap '[ -x "$G" ] && "$G" hub stop --home "$H" > /dev/null 2>&1; for p in $PIDS; do kill $p 2>/dev/null; done; [ -z "${RC_LOGS:-}" ] || { mkdir -p "$RC_LOGS"; cp "$T"/*.log "$RC_LOGS"/ 2>/dev/null; cp "$H/hub.log" "$RC_LOGS/hub-home.log" 2>/dev/null; }; [ -n "${RC_KEEP:-}" ] || rm -rf "$T"' EXIT
echo '[]' > "$T/feed/releases.json"
(cd "$T/feed" && exec "$PY" -I -m http.server $FP --bind 127.0.0.1) > "$T/feed.log" 2>&1 & PIDS="$PIDS $!"
(cd "$ROOT/test/cef/page" && exec "$PY" -I -m http.server $PP --bind 127.0.0.1) > "$T/page.log" 2>&1 & PIDS="$PIDS $!"
# a project and a thread for the client to open, as a person's first project would be
py - "$H/events.jsonl" "$T/proj" <<'PY'
import json, sys, time
now = int(time.time()); out, proj = sys.argv[1], sys.argv[2]
ev = [{"$": "ProjectCreated", "id": "p1", "title": "smoke", "root": proj, "at": now},
      {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "browser", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now}]
# setup done (or the window opens on the first-run steps and never shows the thread), and no carrying on of turns
ev += [{"$": "SettingSet", "key": k, "value": v, "at": now} for k, v in (("onboard.done", "1"), ("onboard.uber", "1"), ("restart.continue", "off"))]
open(out, "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
PY
hello() { curl -s -m3 "http://127.0.0.1:$P/hello" | py "$HERE/cbor_hello.py"; }
until_() { local n=$1; shift; local i=0; while [ $i -lt $n ]; do "$@" && return 0; sleep 1; i=$((i+1)); done; return 1; }

# 1. install as a person would, and the guard starts the hub
if [ -z "$win" ]; then
  A=$REL/backplane-$TAG-$TGT-native.tar.gz
  env BACKPLANE_ARCHIVE="$A" BACKPLANE_PREFIX="$PREFIX" BACKPLANE_BIN="$T/bin" BACKPLANE_HOME="$H" BACKPLANE_PORT=$P BACKPLANE_START=1 BACKPLANE_NO_DESKTOP=1 \
    BACKPLANE_HUB_ARGS=--no-tailscale sh "$REL/install.sh" --quiet > "$T/install.log" 2>&1 || no install "install.sh: $(tail -2 "$T/install.log")"
else
  Z=$T/zip; mkdir -p "$Z"; (cd "$Z" && unzip -q "$REL/backplane-$TAG-$TGT-native.zip") || no install "unzip failed"
  "$Z/backplane-gpui.exe" --install "$(cygpath -w "$PREFIX")" > "$T/install.log" 2>&1; rc=$?
  [ $rc = 0 ] || no install "backplane-gpui.exe --install: exit $rc $(tail -2 "$T/install.log")"
  "$G" hub run --home "$(cygpath -w "$H")" --port $P --host 127.0.0.1 --builds "$(cygpath -w "$PREFIX")" --log "$(cygpath -w "$H/hub.log")" -- --no-tailscale --foreground > "$T/guard.log" 2>&1 &
fi
B=$PREFIX/builds/$TAG
said() { local h; h=$(hello); [ "${h%% *}" = "$VER" ] && [ -n "${h#* }" ] && case "$COMMIT" in "${h#* }"*) true;; *) false;; esac; }
if until_ 120 said; then ok install "installed into builds/$TAG; the guard started the hub; /hello: $(hello)"
else no install "/hello said '$(hello)', wanted $VER $COMMIT; $(tail -3 "$T/install.log" 2>/dev/null)"; fi

# 2. the helpers a dev cut has, each answering
[ -x "$B/backplane-voice$X" ] && "$B/backplane-voice$X" local-status --local-dir "$T/voice-models" > "$T/voice.out" 2>&1 && grep -q '"event":"local"' "$T/voice.out" && grep -q '"vadBundled":true' "$T/voice.out" && ok voice "backplane-voice local-status: $(head -c 120 "$T/voice.out" | tr '\n' ' ')" || no voice "backplane-voice local-status failed, or the bundled VAD (silero_vad.onnx) is missing or not the pinned one: $(head -c 200 "$T/voice.out" 2>/dev/null)"
if "$B/backplane-step2glb$X" "$ROOT/test/files/fixtures/mesh/bracket.step" "$T/bracket.glb" > "$T/step.out" 2>&1 && [ -s "$T/bracket.glb" ] && [ "$(head -c 4 "$T/bracket.glb")" = glTF ]; then ok step2glb "bracket.step -> $(wc -c < "$T/bracket.glb") bytes of GLB"
else no step2glb "$(tail -2 "$T/step.out")"; fi
[ -x "$B/backplane-browser$X" ] && ok browser "backplane-browser beside the hub" || no browser "no backplane-browser$X in builds/$TAG"

# 3. the client against the hub, the Browser tab on a local page; it must draw, with every asset there
cat > "$T/script.json" <<JSON
[{"wait":2500},{"click":"thread:t1"},{"wait":2500},{"click":"rail:Browser"},{"wait":3000},{"cef":{"goto":"http://127.0.0.1:$PP/index.html"}},{"wait":6000},{"cef":{"report":true}},{"wait":1000}]
JSON
run_client() {
  local c=("$B/backplane-gpui$X" --url "http://127.0.0.1:$P" --token-file "$H/token" --exit-after 30)
  case $TGT in
    linux-*)   # software Vulkan where the machine has no GPU (a CI runner): lavapipe
      [ -n "${VK_ICD_FILENAMES:-}" ] || export VK_ICD_FILENAMES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1)
      # a private display with a window manager (without one nothing is drawn); XVFB names an unpacked Xvfb where none is installed
      local d=$((300 + $$ % 300)) r; r=$(mktemp -d /tmp/bpr.XXXX); chmod 700 "$r"   # sockets only: a short path (i3 fails past 107 bytes)
      ${XVFB:-Xvfb} :$d -screen 0 1400x900x24 -nolisten tcp > "$T/xvfb.log" 2>&1 & local xp=$!; sleep 1
      printf 'new_window none\nfor_window [class=".*"] border none\nipc-socket %s/i3.sock\n' "$r" > "$T/i3.conf"
      DISPLAY=:$d XDG_RUNTIME_DIR=$r i3 -c "$T/i3.conf" > "$T/i3.log" 2>&1 & local ip=$!; sleep 1
      DISPLAY=:$d XDG_RUNTIME_DIR=$r BACKPLANE_GPUI_SCRIPT="$T/script.json" "${c[@]}"; kill $ip $xp 2>/dev/null ;;
    windows-x64) BACKPLANE_GPUI_SCRIPT="$(cygpath -w "$T/script.json")" "${c[@]}" ;;
    *) BACKPLANE_GPUI_SCRIPT="$T/script.json" "${c[@]}" ;;
  esac
}
# (no `timeout` on macOS: the client is given --exit-after, and a watchdog ends it if it hangs)
run_client > "$T/client.log" 2>&1 & cp=$!; ( sleep 120; kill $cp 2>/dev/null ) & wd=$!; wait $cp; kill $wd 2>/dev/null
if grep -q 'not in icons.json\|"asset_missing"' "$T/client.log"; then no assets "$(grep -o 'icon [a-z0-9-]* not in icons.json\|asset_missing[^}]*' "$T/client.log" | sort -u | head -5 | tr '\n' ' ')"
elif grep -q '"event":"usable"' "$T/client.log"; then ok assets "the client drew with every icon and cat it asked for"
else no assets "the client never drew ($(tail -3 "$T/client.log" | tr '\n' ' ' | cut -c1-200))"; fi
rep=$(grep -o '"event":"cef"[^}]*\|GPUI_METRIC {[^}]*"title":"[^"]*"[^}]*}' "$T/client.log" | tail -1)
case $TGT in
  linux-*)
    if grep -q 'GPUI_METRIC.*"title":"cef-test-start"' "$T/client.log" && ! grep -q 'embedded browser unavailable' "$T/client.log"; then ok browser-tab "the embedded Chromium loaded and painted the local page: $(grep -o '"paints":[0-9]*' "$T/client.log" | tail -1)"
    else no browser-tab "no painted page (embedded browser: $(grep -o 'embedded browser unavailable[^"]*\|"cef":"[^"]*' "$T/client.log" | head -1)); last report: ${rep:0:200}"; fi ;;
  *)
    # no CEF here (docs/install.md): the Browser tab opens on the hub's frame stream; its frames from backplane-browser are F18's, run by this platform's lane
    if grep '"event":"cef"' "$T/client.log" | grep -q 'built without the cef feature' && grep -q '"event":"usable"' "$T/client.log" && ! grep -q -i 'panicked' "$T/client.log"; then ok browser-tab "the Browser tab opened in frame-stream mode (no CEF on $TGT, as documented); its frames are the F18 row's"
    else no browser-tab "the Browser tab did not open: $(grep -o '"event":"cef"[^}]*' "$T/client.log" | tail -1 | cut -c1-160) $(grep -i panicked "$T/client.log" | head -1 | cut -c1-120)"; fi ;;
esac
"$G" hub stop --home "$H" > /dev/null 2>&1

echo "rc-smoke $TGT $TAG: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
