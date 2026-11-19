#!/bin/bash
# Feature replyImages on an iOS simulator on the Mac, headless, through real gestures (XCUITest ReplyImagesE2E/test_reply_images),
# against a temp-home hub on this machine.
#   test/e2e/ios/reply-images.sh HUB_BIN [OUT]
# Env:
#   MAC_DIR      folder on hs-mac-mini (default bp-reply-images) where ios/build/dd was built by scripts/mobile/ios-check.sh
#   SHOTS_DIR    optional dir of real PNGs (before-project.png, ...); when unset, crisp synthetic PNGs are generated in a mktemp dir
#   MSG_FILE     optional markdown file for the main reply (default test/e2e/ios/reply-images.md; $SHOTS is expanded)
#   CHECK_UNAVAILABLE  1 (default) to also seed a second message with one valid image and one missing image
#   PORT         hub loopback port (default 3863)
#   SIM          simulator device name (default bp-reply-images)
#   MAC_SSH      ssh wrapper (defaults to worktree or main build/gpui/mac-ssh)
set -euo pipefail

HUB=$(realpath "${1:?usage: reply-images.sh HUB_BIN [OUT]}")
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"
. "$ROOT/test/e2e/lib/own-group.sh"

: "${BP_TMP:?BP_TMP must be set}"
OUT=${2:-$ROOT/build/e2e-ios-reply-images}
: "${OUT:?OUT must be set}"
case "$OUT" in
  ""|"/"|"$HOME"|"$HOME/")
    echo "reply-images.sh: refusing unsafe OUT=$OUT" >&2
    exit 1
    ;;
esac
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
case "$OUT" in
  ""|"/"|"$HOME"|"$HOME/")
    echo "reply-images.sh: refusing unsafe resolved OUT=$OUT" >&2
    exit 1
    ;;
esac

PORT=${PORT:-3863}
MAC_DIR=${MAC_DIR:-bp-reply-images}
SIM=${SIM:-bp-reply-images}
CHECK_UNAVAILABLE=${CHECK_UNAVAILABLE:-1}
MSG_FILE=${MSG_FILE:-$ROOT/test/e2e/ios/reply-images.md}
M=${MAC_SSH:-$ROOT/build/gpui/mac-ssh}
[ -x "$M" ] || M=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh
[ -x "$M" ] || { echo "reply-images.sh: missing executable MAC_SSH=$M" >&2; exit 1; }

WORK=$(mktemp -d "$BP_TMP/reply-images.XXXXXX")
: "${WORK:?}"
H="$WORK/hub"
SHOTS="$WORK/shots"
mkdir -p "$H" "$SHOTS"
SHOTS=$(realpath "$SHOTS")

pids=()
stop() {
  for p in "${pids[@]:-}"; do
    [ -n "$p" ] && kill "$p" 2>/dev/null || true
  done
  own_stop 2>/dev/null || true
  case "$WORK" in
    "$BP_TMP"/reply-images.*) rm -rf "$WORK" ;;
  esac
}
trap stop EXIT
trap 'exit 143' INT TERM HUP

# Prepare fixture images: either copy from SHOTS_DIR (so canonical path has no symlinks) or generate crisp PNGs.
if [ -n "${SHOTS_DIR:-}" ]; then
  [ -d "$SHOTS_DIR" ] || { echo "reply-images.sh: SHOTS_DIR not a dir: $SHOTS_DIR" >&2; exit 1; }
  for f in "$SHOTS_DIR"/*.png "$SHOTS_DIR"/*.jpg "$SHOTS_DIR"/*.jpeg; do
    [ -f "$f" ] && cp "$f" "$SHOTS/$(basename "$f")"
  done
else
  python3 - "$SHOTS" <<'PY'
import os, struct, sys, zlib
out = sys.argv[1]
def write_png(path, w, h, rgb, stripe):
    rows = []
    for y in range(h):
        c = stripe if (y // 40) % 2 == 0 and 80 < y < h - 80 else rgb
        rows.append(b"\x00" + bytes(c) * w)
    raw = b"".join(rows)
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)

specs = [
    ("before-project.png",       (46, 58, 82),   (110, 145, 210)),
    ("after-project.png",        (34, 78, 64),   (90, 195, 155)),
    ("after-project-boards.png", (76, 52, 92),   (175, 125, 215)),
    ("after-project-intent.png", (92, 64, 38),   (220, 160, 95)),
    ("after-project-history.png",(54, 72, 96),   (135, 180, 230)),
    ("before-picker.png",        (88, 42, 52),   (215, 110, 130)),
    ("after-picker.png",         (38, 86, 92),   (100, 210, 220)),
    ("after-picker-effort.png",  (64, 78, 40),   (165, 200, 105)),
    ("single.png",               (48, 66, 88),   (130, 175, 225)),
]
for name, bg, fg in specs:
    write_png(os.path.join(out, name), 390, 640, bg, fg)
PY
fi

# Ensure single.png exists if CHECK_UNAVAILABLE=1
if [ "$CHECK_UNAVAILABLE" = "1" ] && [ ! -f "$SHOTS/single.png" ]; then
  first_png=$(find "$SHOTS" -maxdepth 1 -name '*.png' | sort | head -1 || true)
  [ -n "$first_png" ] && cp "$first_png" "$SHOTS/single.png"
fi

mkdir -p "$H/home" "$H/proj" "$H/bin"
for a in claude codex grok muse; do
  printf '#!/bin/sh\nexit 1\n' > "$H/bin/$a"
  chmod 755 "$H/bin/$a"
done

python3 - "$H/home/events.jsonl" "$H/proj" "$MSG_FILE" "$SHOTS" "$CHECK_UNAVAILABLE" <<'PY'
import json, sys, time
out_path, proj_dir, msg_file, shots_dir, check_unavail = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5] == "1"
now = int(time.time())
text = open(msg_file, encoding="utf-8").read().replace("$SHOTS", shots_dir).strip()
ev = [
    {"$": "ProjectCreated", "id": "p1", "title": "demo", "root": proj_dir, "at": now},
    {"$": "ThreadCreated", "id": "timgs", "project": "p1", "title": "Status on iOS?", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
    {"$": "MessagePosted", "thread": "timgs", "msg": "mi1", "role": "user", "text": "Status on iOS?", "at": now},
    {"$": "MessagePosted", "thread": "timgs", "msg": "mi2", "role": "assistant", "text": text, "at": now + 1},
]
if check_unavail:
    ev.append({
        "$": "MessagePosted", "thread": "timgs", "msg": "mi3", "role": "assistant",
        "text": f"Single inline shot:\n{shots_dir}/single.png\n\nAnd a missing file:\n{shots_dir}/missing.png",
        "at": now + 2,
    })
with open(out_path, "w", encoding="utf-8") as f:
    for e in ev:
        f.write(json.dumps(e, separators=(",", ":")) + "\n")
PY

BACKPLANE_AGENTS="$H/bin" BACKPLANE_NO_UPDATE=1 own "$HUB" --home "$H/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.out" 2>&1
hub_pid=$!
pids+=("$hub_pid")
ready=0
for _ in $(seq 1 40); do
  if curl -fsS -m 1 "http://127.0.0.1:$PORT/hello" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.25
done
if [ "$ready" != "1" ] || [ ! -f "$H/home/token" ]; then
  echo "reply-images.sh: temp hub did not become ready on port $PORT" >&2
  cat "$OUT/hub.out" >&2 || true
  exit 1
fi

"$M" -N -o ExitOnForwardFailure=yes -R "$PORT:127.0.0.1:$PORT" hs-mac-mini &
pids+=($!)
sleep 3
kill -0 "${pids[-1]}" 2>/dev/null || { echo "reply-images.sh: ssh tunnel did not start" >&2; exit 1; }

TOK=$(cat "$H/home/token")
RSYNC_RSH="$M" rsync -a --delete "$ROOT/test/e2e/ios/" "hs-mac-mini:$MAC_DIR/e2e/"

EXPECT_TOTAL=${EXPECT_TOTAL:-8}
EXPECT_CAROUSEL=${EXPECT_CAROUSEL:-"1 / 5"}
EXPECT_CAROUSEL_NEXT=${EXPECT_CAROUSEL_NEXT:-"2 / 5"}

rc=0
"$M" hs-mac-mini "
set -euo pipefail
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd \"\$HOME/$MAC_DIR\"
U=\$(xcrun simctl list devices available | grep -E '^ +$SIM \(' | grep -o '[0-9A-F-]\{36\}' | head -1 || true)
[ -n \"\$U\" ] || U=\$(xcrun simctl create '$SIM' 'iPhone 17 Pro')
echo \"simulator \$U\"
xcrun simctl boot \"\$U\" 2>/dev/null || true
xcrun simctl bootstatus \"\$U\" -b >/dev/null 2>&1
xcrun simctl uninstall \"\$U\" dev.backplane.gpui 2>/dev/null || true
xcrun simctl install \"\$U\" ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app
RUN_DIR=\$(mktemp -d \"\$HOME/$MAC_DIR/ri-out.XXXXXX\")
printf %s '$TOK' > \"\$RUN_DIR/token\"
mkdir -p \"\$RUN_DIR/out\"
nice -n 10 xcodebuild build-for-testing -quiet \
  -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator \
  -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd \
  CODE_SIGNING_ALLOWED=NO > \"\$RUN_DIR/out/build.log\" 2>&1 || { grep -E 'error:' \"\$RUN_DIR/out/build.log\" | head -20; exit 4; }
test_rc=0
TEST_RUNNER_HUB_HOST=127.0.0.1 \
TEST_RUNNER_HUB_PORT=$PORT \
TEST_RUNNER_HUB_TOKEN_FILE=\"\$RUN_DIR/token\" \
TEST_RUNNER_OUT=\"\$RUN_DIR/out\" \
TEST_RUNNER_EXPECT_TOTAL='$EXPECT_TOTAL' \
TEST_RUNNER_EXPECT_CAROUSEL='$EXPECT_CAROUSEL' \
TEST_RUNNER_EXPECT_CAROUSEL_NEXT='$EXPECT_CAROUSEL_NEXT' \
TEST_RUNNER_CHECK_UNAVAILABLE='$CHECK_UNAVAILABLE' \
  xcodebuild test-without-building -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator \
  -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd \
  CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/ReplyImagesE2E/test_reply_images > \"\$RUN_DIR/out/xcodebuild.log\" 2>&1 || test_rc=\$?
rm -f \"\$RUN_DIR/token\"
xcrun simctl shutdown \"\$U\" 2>/dev/null || true
echo \"REMOTE_OUT=\$RUN_DIR/out\"
exit \$test_rc
" 2>&1 | tee "$OUT/run.log" || rc=${PIPESTATUS[0]}

REMOTE_OUT=$(grep '^REMOTE_OUT=' "$OUT/run.log" | tail -1 | cut -d= -f2- || true)
if [ -n "$REMOTE_OUT" ]; then
  RSYNC_RSH="$M" rsync -a "hs-mac-mini:$REMOTE_OUT/" "$OUT/"
  REMOTE_PARENT=$(dirname "$REMOTE_OUT")
  case "$REMOTE_PARENT" in
    *"/$MAC_DIR/ri-out."*) "$M" hs-mac-mini "rm -rf '$REMOTE_PARENT'" || true ;;
  esac
fi

python3 - "$OUT/reply-images.json" <<'PY' 2>&1 | tee -a "$OUT/run.log"
import json, sys
try:
    r = json.load(open(sys.argv[1]))
except Exception:
    print("no reply-images.json: the test did not run")
    sys.exit(0)
print("status", r.get("status"))
for s in r["metrics"]["steps"]:
    print(" ", "ok  " if s.get("ok") else "FAIL", s["do"])
for n in r.get("notes", []):
    print("  note", n[:200])
PY

[ -f "$OUT/reply-images.json" ] || { [ "$rc" = 0 ] && rc=5; }
echo "xcodebuild exit $rc"
exit "$rc"
