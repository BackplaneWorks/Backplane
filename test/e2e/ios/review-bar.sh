#!/bin/bash
# The review bar (#1305) on an iOS simulator on the Mac, headless, through real taps (XCUITest ReviewBarE2E/test_review_bar), against a temp-home
# hub on this machine (loopback, reached from the Mac through an ssh tunnel; stand-in agents only). The hub's log must show each press.
#   test/e2e/ios/review-bar.sh HUB_BIN [OUT]     env: MAC_DIR (default bp-review-bar: ios/build/dd built there first), PORT (3864), SIM (bp-review-bar), MAC_SSH
set -euo pipefail
HUB=$(realpath "${1:?usage: review-bar.sh HUB_BIN [OUT]}")
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"; . "$ROOT/test/e2e/lib/own-group.sh"
OUT=${2:-$ROOT/build/e2e-ios-review-bar}; mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
case "$OUT" in ""|"/"|"$HOME"|"$HOME/") echo "review-bar.sh: refusing OUT=$OUT" >&2; exit 1;; esac
PORT=${PORT:-3864}; MAC_DIR=${MAC_DIR:-bp-review-bar}; SIM=${SIM:-bp-review-bar}
M=${MAC_SSH:-$ROOT/build/gpui/mac-ssh}; [ -x "$M" ] || M=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh
WORK=$(mktemp -d "$BP_TMP/review-bar.XXXXXX"); H=$WORK/hub; mkdir -p "$H/home" "$H/proj" "$H/bin"
pids=(); stop() { for p in "${pids[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null || true; done; own_stop 2>/dev/null || true; case "$WORK" in "$BP_TMP"/review-bar.*) rm -rf "$WORK";; esac; }
trap stop EXIT; trap 'exit 143' INT TERM HUP
for a in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$H/bin/$a"; chmod 755 "$H/bin/$a"; done
python3 - "$H/home/events.jsonl" "$H/proj" <<'PY'
import json, sys, time
out, proj = sys.argv[1], sys.argv[2]; now = int(time.time())
ev = [{"$": "ProjectCreated", "id": "p1", "title": "demo", "root": proj, "at": now},
      {"$": "ThreadCreated", "id": "trev", "project": "p1", "title": "Board review", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
      {"$": "MessagePosted", "thread": "trev", "msg": "mr1", "role": "user", "text": "review the board", "at": now},
      {"$": "SettingSet", "key": "review.ro.trev", "value": "1", "at": now},
      {"$": "MessagePosted", "thread": "trev", "msg": "mr2", "role": "assistant", "text": "The board looks sound. Two things I would change: C4 is far from U1's VDD pin, and the USB pair has a stub at J1.", "at": now + 1}]
open(out, "w").write("".join(json.dumps(e) + "\n" for e in ev))
PY
BACKPLANE_AGENTS="$H/bin" BACKPLANE_NO_UPDATE=1 own "$HUB" --home "$H/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.out" 2>&1
pids+=($!)
for _ in $(seq 1 40); do curl -fsS -m 1 "http://127.0.0.1:$PORT/hello" >/dev/null 2>&1 && break; sleep 0.25; done
[ -f "$H/home/token" ] || { echo "review-bar.sh: the hub did not start" >&2; cat "$OUT/hub.out" >&2; exit 1; }
"$M" -N -o ExitOnForwardFailure=yes -R "$PORT:127.0.0.1:$PORT" hs-mac-mini & pids+=($!); sleep 3
TOK=$(cat "$H/home/token")
RSYNC_RSH="$M" rsync -a --delete --exclude build "$ROOT/mobile/ios/" "hs-mac-mini:$MAC_DIR/ios/"
RSYNC_RSH="$M" rsync -a --delete "$ROOT/test/e2e/ios/" "hs-mac-mini:$MAC_DIR/e2e/"
rc=0
"$M" hs-mac-mini "
set -euo pipefail
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd \"\$HOME/$MAC_DIR\"
lockf -k /tmp/bp-mac-heavy.lockf sh -c '
set -e
cd ios && xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:\" | head -30 || true
cd ..
U=\$(xcrun simctl list devices available | grep -E \"^ +$SIM \\(\" | grep -o \"[0-9A-F-]\\{36\\}\" | head -1 || true)
[ -n \"\$U\" ] || U=\$(xcrun simctl create $SIM \"iPhone 17 Pro\")
xcrun simctl boot \"\$U\" 2>/dev/null || true; xcrun simctl bootstatus \"\$U\" -b >/dev/null 2>&1
xcrun simctl uninstall \"\$U\" dev.backplane.gpui 2>/dev/null || true
xcrun simctl install \"\$U\" ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
rm -rf out; mkdir -p out; printf %s $TOK > token
xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > out/build.log 2>&1 || { grep -E \"error:\" out/build.log | head -20; exit 4; }
r=0
TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/token TEST_RUNNER_OUT=\$HOME/$MAC_DIR/out \
  xcodebuild test-without-building -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd \
  CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/ReviewBarE2E/test_review_bar > out/xcodebuild.log 2>&1 || r=\$?
rm -f token; xcrun simctl shutdown \"\$U\" 2>/dev/null || true; exit \$r
'
" 2>&1 | tee "$OUT/run.log" || rc=${PIPESTATUS[0]}
RSYNC_RSH="$M" rsync -a --exclude '*.xcresult' "hs-mac-mini:$MAC_DIR/out/" "$OUT/" || true
python3 - "$OUT/review-bar.json" "$H/home/events.jsonl" <<'PY' | tee -a "$OUT/run.log"
import json, sys
try: r = json.load(open(sys.argv[1]))
except Exception: print("no review-bar.json: the test did not run"); sys.exit(0)
print("status", r.get("status"))
for s in r["metrics"]["steps"]: print(" ", "ok  " if s.get("ok") else "FAIL", s["do"])
vals = [json.loads(l).get("value") for l in open(sys.argv[2]) if '"review.ro.trev"' in l]
print("hub: review.ro.trev took", vals, "ok" if vals[-3:] == ["1", "0", "1"] else "FAIL")
PY
[ -f "$OUT/review-bar.json" ] || { [ "$rc" = 0 ] && rc=5; }
grep -q "^hub: .* ok$" "$OUT/run.log" || { [ "$rc" = 0 ] && rc=6; }
echo "exit $rc"; exit "$rc"
