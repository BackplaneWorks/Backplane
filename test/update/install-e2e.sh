#!/bin/bash
# A fresh install from release dry runs, end to end: install.sh (as release.yml filled it in) lays the release out and starts the guard, which starts the
# hub; a newer signed release on a loopback feed updates it; a release that panics at its start is rolled back. Private HOME, prefix, home and port.
#   test/update/install-e2e.sh FIRST NEXT BROKEN
#     each a folder holding one dry run's artifact (release-dry-run: archives, install.sh, manifest.json, .sig, .keyid), all on the beta channel,
#     FIRST < NEXT < BROKEN; BROKEN's hub must fail its start. Signed with the real key by release.yml; nothing here signs anything.
set -u
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
FIRST=$(cd "$1" && pwd); NEXT=$(cd "$2" && pwd); BROKEN=$(cd "$3" && pwd)
PY=$(command -v python3 || command -v python)   # Windows runners name it python
python3() { "$PY" "$@"; }
tag() { python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["tag"])' "$1/manifest.json"; }
T1=$(tag "$FIRST"); T2=$(tag "$NEXT"); T3=$(tag "$BROKEN")
case "$(uname -s)-$(uname -m)" in Linux-x86_64) TGT=linux-x64;; Linux-aarch64) TGT=linux-arm64;; Darwin-arm64) TGT=darwin-arm64;; MINGW*|MSYS*|CYGWIN*) TGT=windows-x64;; *) echo "no target"; exit 2;; esac
win=; X=; [ "$TGT" = windows-x64 ] && { win=1; X=.exe; }
w() { if [ -n "$win" ]; then cygpath -w "$1"; else printf '%s' "$1"; fi; }   # a path as the Windows programs take it
T=$(mktemp -d "${UPD_DIR:-$ROOT/build}/.install-e2e.XXXXXX"); P=${UPD_PORT:-3995}; FP=$((P+500)); F=$T/feed; H=$T/home; PASS=0; FAILS=()
export HOME=$T/userhome BACKPLANE_PREFIX=$T/prefix BACKPLANE_BIN=$T/bin BACKPLANE_HOME=$H BACKPLANE_PORT=$P BACKPLANE_NO_DESKTOP_APP=1 BACKPLANE_NO_DESKTOP=1 \
  BACKPLANE_HUB_ARGS=--no-tailscale BACKPLANE_AGENTS=$T/agents BACKPLANE_UPDATE_FEED=http://127.0.0.1:$FP/releases.json BACKPLANE_GUARD_STABLE=10
unset BACKPLANE_COPY BACKPLANE_NO_UPDATE BACKPLANE_BUILDS BACKPLANE_GUARD BACKPLANE_E2E_TRUST_KEY BACKPLANE_AGENT_OWNER BACKPLANE_FAULT BACKPLANE_SAFE
mkdir -p "$HOME" "$T/agents" "$F" "$H"
G=$BACKPLANE_PREFIX/backplane$X
[ -z "$win" ] || export USERPROFILE="$(w "$HOME")" LOCALAPPDATA="$(w "$HOME")"
stop() { [ -x "$G" ] && "$G" hub stop --home "$(w "$H")" > /dev/null 2>&1; [ -n "${FPID:-}" ] && kill "$FPID" 2>/dev/null; }
trap 'stop; [ -n "${UPD_KEEP:-}" ] || rm -rf "${T:?}"' EXIT
ok() { PASS=$((PASS+1)); printf 'PASS %-9s %s\n' "$1" "$2"; }
no() { FAILS+=("$1"); printf 'FAIL %-9s %s\n' "$1" "$2"; tail -6 "$H/hub.log" 2>/dev/null | cut -c1-180 | sed 's/^/    /'; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*2)) ]; do if "$@"; then return 0; fi; sleep 0.5; i=$((i+1)); done; return 1; }
st() { python3 -I -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get(sys.argv[2],""))
except Exception: pass' "$H/guard/status.json" "$1"; }
hello() { curl -s -m3 "http://127.0.0.1:$P/hello" | python3 -I "$HERE/cbor_hello.py"; }
commit_of() { python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["commit"])' "$1/manifest.json"; }
says() { local h; h=$(hello); [ "${h%% *}" = "${2#v}" ] && case "$(commit_of "$1")" in "${h#* }"*) true;; *) false;; esac; }
on() { [ "$(st build)" = "$2" ] && [ "$(st stable)" = True ] && says "$1" "$2"; }
# the feed: the releases given, as GitHub lists them, served on loopback
feed() { for d in "$@"; do t=$(tag "$d"); mkdir -p "$F/$t"; cp "$d"/manifest.json* "$d"/backplane-"$t"-$TGT-native.* "$F/$t/"; done
  python3 -I - "$F" "$FP" "$@" > "$F/releases.json" <<'PY'
import json, os, sys
f, port = sys.argv[1], sys.argv[2]
tags = [json.load(open(os.path.join(d, "manifest.json")))["tag"] for d in sys.argv[3:]]
print(json.dumps([{"tag_name": t, "prerelease": "-" in t, "draft": False, "assets": [{"name": n, "browser_download_url": f"http://127.0.0.1:{port}/{t}/{n}"} for n in sorted(os.listdir(os.path.join(f, t)))]} for t in tags]))
PY
}
(cd "$F" && exec python3 -I -m http.server "$FP" --bind 127.0.0.1) > "$T/feed.log" 2>&1 & FPID=$!
# the beta channel, set before the first start as a person would set it in Settings
printf '%s\n' '{"$":"SettingSet","key":"update.channel","value":"beta","at":1}' > "$H/events.jsonl"
feed "$NEXT"   # on the feed from the start: the hub's first check takes it

# 1. install: the release laid out, the guard started, the guard starting the hub
if [ -z "$win" ]; then
  BACKPLANE_ARCHIVE="$FIRST/backplane-$T1-$TGT-native.tar.gz" sh "$FIRST/install.sh" --quiet > "$T/install.log" 2>&1 < /dev/null; rc=$?
else
  # Windows: the zip unpacked and installed by its own client (native/gpui/src/install.rs), then the guard on the private home and port
  mkdir -p "$T/zip" && (cd "$T/zip" && unzip -q "$FIRST/backplane-$T1-$TGT-native.zip") && "$T/zip/backplane-gpui.exe" --install "$(w "$BACKPLANE_PREFIX")" > "$T/install.log" 2>&1; rc=$?
  [ $rc != 0 ] || { "$G" hub run --home "$(w "$H")" --port $P --host 127.0.0.1 --builds "$(w "$BACKPLANE_PREFIX")" --log "$(w "$H/hub.log")" -- --no-tailscale --foreground > "$T/guard.log" 2>&1 & }
fi
if [ $rc != 0 ]; then no install "install failed ($rc): $(tail -2 "$T/install.log")"
elif ! [ -x "$G" ] || ! [ -x "$BACKPLANE_PREFIX/builds/$T1/backplane-hub$X" ] || [ "$(tr -d '\r' < "$BACKPLANE_PREFIX/CURRENT")" != "$T1" ] || { [ -z "$win" ] && ! [ -L "$BACKPLANE_BIN/backplane" ]; }; then no install "layout: $(ls "$BACKPLANE_PREFIX" "$BACKPLANE_PREFIX/builds" 2>&1 | tr '\n' ' ')"
elif ! until_ 90 eval '[ "$(st build)" = "$T1" ] && says "$FIRST" "$T1"'; then no install "the guard did not bring up $T1 (state $(st state), /hello $(hello))"
elif [ "$(st hubPid)" = 0 ] || [ -z "$(st hubPid)" ]; then no install "no hub under the guard"
else ok install "$([ -n "$win" ] && echo "the zip's --install" || echo install.sh) laid out builds/$T1, CURRENT and the guard; the guard started the hub: /hello $(hello)"; fi

# 2. update: the hub's own first check finds the next signed release; the guard runs it on probation and proves it
if ! until_ 240 on "$NEXT" "$T2"; then no update "not on $T2 (on $(st build), state $(st state), BAD: $(cat "$BACKPLANE_PREFIX/BAD" 2>/dev/null))"
elif [ "$(tr -d '\r' < "$BACKPLANE_PREFIX/CURRENT")" != "$T2" ]; then no update "proven, but CURRENT says $(cat "$BACKPLANE_PREFIX/CURRENT")"
else ok update "$T1 -> $T2 from the loopback feed: verified against release.pub, staged, proven, CURRENT moved; /hello $(hello)"; fi

# 3. rollback: a signed release whose hub dies at its start goes straight back and is marked bad
feed "$NEXT" "$BROKEN"; "$G" hub restart --home "$(w "$H")" --now > /dev/null 2>&1   # a restart checks again at once (the next check is otherwise an hour off)
if ! until_ 300 eval 'grep -q "^$T3" "$BACKPLANE_PREFIX/BAD" 2>/dev/null'; then no rollback "$T3 never marked bad (on $(st build), state $(st state))"
elif ! until_ 120 on "$NEXT" "$T2"; then no rollback "not back on $T2 (on $(st build), /hello $(hello))"
else ok rollback "$T3 died at its start: marked bad ($(grep "^$T3" "$BACKPLANE_PREFIX/BAD" | cut -f2 | cut -c1-80)), back on $T2"; fi

echo "install E2E: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
