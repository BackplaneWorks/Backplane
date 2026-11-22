#!/bin/bash
# The desktop app's own updates where no guard runs (docs/deploy.md, "A desktop app on another machine's hub"), end to end: an install whose app uses a hub
# on another machine (launch.json names a url), a release feed served on loopback and signed with a keypair made for this run, the real client in a private
# Xvfb. Every release is below 0.1.0, the ceiling the e2e-trust feature holds a test key to. Nothing of the person's: its own HOME, prefix, config and display.
#   A (v0.0.1) installed and running stages B (v0.0.2): next, EXPECT, A left as it was, CURRENT still A
#   a second app opened while A runs does not move (an app runs: nothing changes under it)
#   the next launch tries B in A's place, B's window opens, CURRENT becomes B, A is kept as the way back
#   a signed C (v0.0.3) whose app dies at its start: staged, tried, not CURRENT; the launch after it marks C bad, B opens, C is never offered again
#   test/update/app-e2e.sh GUARD GPUI
#     GUARD: the guard (`backplane`) built with `--features e2e-trust`; GPUI: the desktop app (backplane-gpui)
# Env: UPD_DIR (scratch, default $BP_TMP or build/), XVFB (an Xvfb), E2E_VK_ICD (a Vulkan driver file; lavapipe by default), APP_SECS (each launch's life, default 12).
set -u
GUARD=$(realpath "${1:?usage: app-e2e.sh GUARD GPUI}"); GPUI=$(realpath "${2:?usage: app-e2e.sh GUARD GPUI}")
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
[ -f "$ROOT/scripts/lib-bptmp.sh" ] && . "$ROOT/scripts/lib-bptmp.sh"
strings "$GUARD" | grep -q backplane-e2e-trust-enabled || { echo "$GUARD was not built with feature e2e-trust"; exit 2; }
case "$(uname -s)-$(uname -m)" in Linux-x86_64) TGT=linux-x64;; Linux-aarch64) TGT=linux-arm64;; *) echo "Linux only (a private Xvfb)"; exit 2;; esac
fp() { python3 -I -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
SCRATCH=${UPD_DIR:-${BP_TMP:-$ROOT/build}}; mkdir -p "$SCRATCH"
T=$(mktemp -d "$SCRATCH/.app-e2e.XXXXXX"); P=$T/prefix; F=$T/feed; FP=$(fp); SECS=${APP_SECS:-12}; PASS=0; FAILS=(); PIDS=""
ICD=${E2E_VK_ICD:-$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1)}
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null; done; rm -rf "${T:?}/keys"; [ -n "${UPD_KEEP:-}" ] || rm -rf "${T:?}"; }
trap cleanup EXIT
mkdir -p "$T/keys" "$T/home" "$T/config/backplane-gpui" "$T/run" "$F" "$P/builds"; chmod 700 "$T/run"
ok() { PASS=$((PASS+1)); printf 'PASS %-9s %s\n' "$1" "$2"; }
no() { FAILS+=("$1"); printf 'FAIL %-9s %s\n' "$1" "$2"; for f in "$T"/launch-*.log "$P/app-update/keep.log"; do [ -f "$f" ] && { echo "  -- $f"; grep -v GPUI_METRIC "$f" | tail -6 | cut -c1-200 | sed 's/^/    /'; }; done; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*4)) ]; do if "$@"; then return 0; fi; sleep 0.25; i=$((i+1)); done; return 1; }
j() { python3 -I -c 'import json,sys
try:
  v=json.load(open(sys.argv[1]))
  for k in sys.argv[2:]: v=v.get(k,"") if isinstance(v,dict) else ""
  print(v)
except Exception: pass' "$@"; }
cur() { tr -d '\r\n' < "$P/CURRENT"; }
nxt() { tr -d '\r\n' 2>/dev/null < "$P/app-update/next"; }
state() { j "$P/app-update/status.json" state; }

# --- the keys: a trusted test key, made now, gone at the end
openssl genpkey -algorithm ed25519 -out "$T/keys/e2e.key" 2>/dev/null && openssl pkey -in "$T/keys/e2e.key" -pubout -out "$T/keys/e2e.pub" || { echo "openssl cannot make ed25519 keys"; exit 2; }
keyid() { openssl pkey -in "$1" -pubout -outform DER | tail -c 32 | sha256sum | cut -c1-16; }

# --- a release: the guard, a stand-in hub (no hub runs here) and the app, or for `broken` an app that dies before its window; a signed manifest naming it
release() { # release TAG [broken]
  local tag=$1 w=$T/w/$1 d=$F/$1; mkdir -p "$w" "$d"
  cp "$GUARD" "$w/backplane"; printf '#!/bin/sh\nexit 0\n' > "$w/backplane-hub"; mkdir -p "$w/assets"; cp -R "$ROOT/assets/." "$w/assets/"
  if [ "${2:-}" = broken ]; then printf '#!/bin/sh\necho "backplane-gpui: broken on purpose" >&2\nexit 1\n' > "$w/backplane-gpui"; else cp "$GPUI" "$w/backplane-gpui"; fi
  chmod +x "$w/backplane" "$w/backplane-hub" "$w/backplane-gpui"
  local a=backplane-$tag-$TGT-native.tar.gz; tar -czf "$d/$a" -C "$w" .
  python3 -I - "$d/manifest.json" "$tag" "$TGT" "$d/$a" <<'PY'
import hashlib, json, os, sys
out, tag, tgt, arc = sys.argv[1:]
b = open(arc, "rb").read()
open(out, "w").write(json.dumps({"schema": 1, "tag": tag, "version": tag[1:], "channel": "stable", "commit": "e2e0000000000000000000000000000000000000",
    "assets": {tgt: {"name": os.path.basename(arc), "sha256": hashlib.sha256(b).hexdigest(), "size": len(b)}}}, indent=1))
PY
  openssl pkeyutl -sign -rawin -inkey "$T/keys/e2e.key" -in "$d/manifest.json" -out "$d/manifest.json.sig"
  printf %s "$(keyid "$T/keys/e2e.key")" > "$d/manifest.json.keyid"
}
publish() { python3 -I - "$F" "$FP" "$@" > "$F/releases.json" <<'PY'
import json, os, sys
root, port, tags = sys.argv[1], sys.argv[2], sys.argv[3:]
print(json.dumps([{"tag_name": t, "prerelease": False, "draft": False,
    "assets": [{"name": n, "browser_download_url": f"http://127.0.0.1:{port}/{t}/{n}"} for n in sorted(os.listdir(os.path.join(root, t)))]} for t in tags]))
PY
}
release v0.0.1; release v0.0.2; release v0.0.3 broken; publish
(cd "$F" && exec python3 -I -m http.server "$FP" --bind 127.0.0.1) > "$T/feed.log" 2>&1 & PIDS="$PIDS $!"

# --- the install, as install.sh lays it out: A unpacked in builds/, CURRENT, the guard beside them (no guard runs: the hub is elsewhere)
mkdir -p "$P/builds/v0.0.1" && tar -xzf "$F/v0.0.1/backplane-v0.0.1-$TGT-native.tar.gz" -C "$P/builds/v0.0.1" && echo v0.0.1 > "$P/CURRENT" && cp "$GUARD" "$P/backplane"
A_SUM=$(cd "$P/builds/v0.0.1" && find . -type f -exec sha256sum {} + | sort | sha256sum)
# this device uses a hub on another machine: an address where nothing answers is enough, the app opens its window and keeps trying
printf 'x\n' > "$T/token"; printf '{"home":"","url":"http://127.0.0.1:%s","token_file":"%s"}\n' "$(fp)" "$T/token" > "$T/config/backplane-gpui/launch.json"

# --- a private display; i3 maps the window where there is one
DISP=:$((300 + $$ % 300)); ${XVFB:-Xvfb} "$DISP" -screen 0 1400x900x24 -nolisten tcp > "$T/xvfb.log" 2>&1 & PIDS="$PIDS $!"; sleep 1
if command -v i3 > /dev/null; then printf 'font pango:monospace 8\nnew_window none\ndefault_border none\n' > "$T/i3.conf"; XDG_RUNTIME_DIR=$T/run DISPLAY=$DISP i3 -c "$T/i3.conf" > "$T/i3.log" 2>&1 & PIDS="$PIDS $!"; sleep 1; fi

# --- a launch, as Backplane.app and backplane-app do it: the app of CURRENT. `next` numbers it first (so one started with `&` has its own log), then
#     `launch [SECS]` runs it; launch-$N.log is what it said.
N=0
next() { N=$((N+1)); }
launch() { # launch [SECS]
  local log=$T/launch-$N.log
  env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$T/home" LANG=C.UTF-8 XDG_RUNTIME_DIR="$T/run" XDG_CONFIG_HOME="$T/config" VK_ICD_FILENAMES="$ICD" DISPLAY="$DISP" \
    BACKPLANE_UPDATE_FEED="http://127.0.0.1:$FP/releases.json" BACKPLANE_E2E_TRUST_KEY="$T/keys/e2e.pub" BACKPLANE_NO_TAILSCALE=1 \
    "$P/builds/$(cur)/backplane-gpui" --exit-after "${1:-$SECS}" > "$log" 2>&1
}
said() { grep -q "$2" "$T/launch-$1.log" 2>/dev/null; }
keep_done() { case "$(state)" in checking|downloading|"") false;; *) true;; esac; }

# 1. A opens, its guard finds B and stages it for the next launch; meanwhile a second app does not move
publish v0.0.2
next; launch 60 & L=$!
if ! until_ 120 eval '[ "$(nxt)" = v0.0.2 ] && [ "$(state)" = ready ]'; then no stage "B never staged (state $(state), next $(nxt))"
elif [ "$(j "$P/builds/v0.0.2/EXPECT" version)" != 0.0.2 ]; then no stage "builds/v0.0.2 has no EXPECT from the signed manifest"
elif [ "$(cur)" != v0.0.1 ] || [ "$(cd "$P/builds/v0.0.1" && find . -type f -exec sha256sum {} + | sort | sha256sum)" != "$A_SUM" ]; then no stage "A was touched (CURRENT $(cur))"
else ok stage "A's own guard staged v0.0.2 from the signed loopback feed: next, EXPECT; A and CURRENT as they were"; fi
if ! kill -0 "$L" 2>/dev/null; then
  no alone "first window ended before concurrent launch check"
else
  next; launch 6; S=$N
  if ! kill -0 "$L" 2>/dev/null; then no alone "first window did not stay alive across second launch"
  elif ! said $S "first_frame"; then no alone "second window never opened its frame"
  elif said $S "trying v0.0.2" || [ "$(cur)" != v0.0.1 ]; then no alone "a second app moved while A ran (CURRENT $(cur))"
  else ok alone "a second app opened its window while A ran and stayed on v0.0.1"; fi
fi
wait $L

# 2. the next launch tries B in A's place; its window opens and it becomes CURRENT
next; launch; n=$N
if ! said $n "trying v0.0.2"; then no switch "the launch did not try v0.0.2"
elif ! said $n "v0.0.2 opened its window"; then no switch "v0.0.2 never opened its window"
elif [ "$(cur)" != v0.0.2 ] || [ -e "$P/app-update/trial" ]; then no switch "CURRENT $(cur), trial $(cat "$P/app-update/trial" 2>/dev/null)"
elif ! [ -x "$P/builds/v0.0.1/backplane-gpui" ]; then no switch "A is gone: no way back"
else ok switch "launched as A, ran as v0.0.2, its window opened: CURRENT is v0.0.2, A kept"; fi
until_ 60 keep_done

# 3. a signed C whose app dies at its start: staged, tried, never CURRENT; the launch after it marks it bad and B opens
publish v0.0.2 v0.0.3
next; launch; until_ 120 eval '[ "$(nxt)" = v0.0.3 ] && [ "$(state)" = ready ]'
next; launch; n=$N
if ! said $n "trying v0.0.3" || [ "$(cur)" != v0.0.2 ] || [ "$(j "$P/app-update/trial" tag)" != v0.0.3 ]; then no broken "C was not tried as a trial (CURRENT $(cur), next $(nxt))"
else
  next; launch; n=$N
  if ! grep -q "^v0.0.3	" "$P/BAD" 2>/dev/null; then no broken "C never marked bad"
  elif [ "$(cur)" != v0.0.2 ] || [ -e "$P/app-update/trial" ] || ! said $n "marked bad"; then no broken "not back on B (CURRENT $(cur))"
  elif ! until_ 60 keep_done || [ "$(nxt)" = v0.0.3 ] || [ "$(state)" != idle ]; then no broken "C offered again (state $(state), next $(nxt))"
  elif [ "$(j "$P/app-update/status.json" said stayed)" != v0.0.3 ]; then no broken "Settings would not say C did not start"
  else ok broken "C died at its start: CURRENT stayed v0.0.2, the next launch marked C bad ($(grep "^v0.0.3" "$P/BAD" | cut -f2)), B opened, C not offered again"; fi
fi

echo "app update E2E: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
