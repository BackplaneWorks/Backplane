#!/bin/bash
# The update E2E: a real guard and a real hub (built with feature e2e-trust) on a private home and port, against a release feed served on loopback and signed
# with a keypair made fresh for this run. Never the real key, never a real version: every release here is below 0.1.0, the ceiling the feature holds a test key to.
#   test/update/feed.sh BIN_DIR [CASE...]
#     BIN_DIR: backplane (the guard) and backplane-hub built with `--features e2e-trust`, and COMMIT, the commit compiled into it (BACKPLANE_COMMIT at build time)
#     CASE: good panic die mismatch older wrongkey ceiling healthz phone hold (none: all but hold). phone runs only with PHONE=<adb serial> (APK=<debug apk> installs it first).
# Env: UPD_DIR (scratch, default BIN_DIR; on disk), UPD_PORT (first port, default 3900).
set -u
BIN=$(cd "${1:?usage: feed.sh BIN_DIR [CASE...]}" && pwd); shift
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$HERE/../..
COMMIT=$(tr -d '[:space:]' < "$BIN/COMMIT"); [ -n "$COMMIT" ] || { echo "no COMMIT in $BIN"; exit 2; }
strings "$BIN/backplane-hub" | grep -q backplane-e2e-trust-enabled || { echo "$BIN/backplane-hub was not built with feature e2e-trust"; exit 2; }
T=$(mktemp -d "${UPD_DIR:-$BIN}/.update-e2e.XXXXXX"); PORT=$((${UPD_PORT:-3900}-1)); PASS=0; FAILS=(); GP=; FP=
case "$(uname -s)-$(uname -m)" in Linux-x86_64) TGT=linux-x64;; Linux-aarch64) TGT=linux-arm64;; Darwin-arm64) TGT=darwin-arm64;; *) echo "no target"; exit 2;; esac
HOME_REAL=$HOME
# nothing of the person's: its own HOME, agents folder and guard pointer; no tailscale
export HOME=$T/userhome BACKPLANE_AGENTS=$T/agents BACKPLANE_GUARD_PROBE=1 BACKPLANE_GUARD_STABLE=3 BACKPLANE_GUARD_PAUSE=0 BACKPLANE_GUARD_MACHINE_PAUSE=1 BACKPLANE_GUARD_RESCUE_RETRY=3
mkdir -p "$HOME" "$T/agents" "$T/keys"; unset BACKPLANE_COPY BACKPLANE_NO_UPDATE BACKPLANE_FAULT BACKPLANE_SAFE BACKPLANE_AGENT_OWNER BACKPLANE_UPDATE_FEED
stopall() { for p in $GP $FP; do kill "$p" 2>/dev/null; done; for p in $GP; do for _ in $(seq 60); do kill -0 "$p" 2>/dev/null || break; sleep 0.25; done; kill -9 "$p" 2>/dev/null; done; GP=; FP=; }
# the keys go with the run, whatever happens; the rest stays only with UPD_KEEP
trap 'stopall; rm -rf "${T:?}/keys"; [ -n "${UPD_KEEP:-}${PHONE:-}" ] || rm -rf "${T:?}"; [ -z "${PHONE:-}" ] || adb -s "$PHONE" reverse --remove-all >/dev/null 2>&1' EXIT

# --- keys: the trusted test key and another one, both made now
openssl genpkey -algorithm ed25519 -out "$T/keys/e2e.key" 2>/dev/null && openssl pkey -in "$T/keys/e2e.key" -pubout -out "$T/keys/e2e.pub" || { echo "openssl cannot make ed25519 keys"; exit 2; }
openssl genpkey -algorithm ed25519 -out "$T/keys/other.key" 2>/dev/null
export BACKPLANE_E2E_TRUST_KEY=$T/keys/e2e.pub
keyid() { openssl pkey -in "$1" -pubout -outform DER | tail -c 32 | sha256sum | cut -c1-16; }

say() { printf '  %s\n' "$*"; }
ok() { PASS=$((PASS+1)); printf 'PASS %-10s %s\n' "$CUR" "$*"; stopall; }
no() { FAILS+=("$CUR"); printf 'FAIL %-10s %s\n' "$CUR" "$*"; tail -8 "$T/$CUR/hub.log" 2>/dev/null | cut -c1-180 | sed 's/^/    /'; stopall; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*4)) ]; do if "$@"; then return 0; fi; sleep 0.25; i=$((i+1)); done; return 1; }

# --- a release: an archive whose backplane-hub is a shell line in front of the real hub (it says `shown` as its version; `fault` breaks it in a real place),
#     a manifest naming `tag`, signed with `key`. The commit is the real one, so only the version can differ from what was signed.
release() { # release TAG SHOWN KEY [FAULT]
  local tag=$1 shown=$2 key=$3 fault=${4:-} d=$T/$CUR/feed/$1 w=$T/$CUR/w/$1
  mkdir -p "$d" "$w"
  printf '#!/bin/sh\n%sexec "%s/backplane-hub" --version-string %s "$@"\n' "${fault:+BACKPLANE_FAULT=$fault }" "$BIN" "$shown" > "$w/backplane-hub"; chmod +x "$w/backplane-hub"
  local a=backplane-$tag-$TGT-native.tar.gz; tar -czf "$d/$a" -C "$w" backplane-hub
  python3 -I - "$d/manifest.json" "$tag" "$COMMIT" "$TGT" "$d/$a" <<'PY'
import hashlib, json, os, sys
out, tag, commit, tgt, arc = sys.argv[1:]
b = open(arc, "rb").read()
open(out, "w").write(json.dumps({"schema": 1, "tag": tag, "version": tag[1:], "channel": "stable", "commit": commit,
    "assets": {tgt: {"name": os.path.basename(arc), "sha256": hashlib.sha256(b).hexdigest(), "size": len(b)}}}, indent=1))
PY
  openssl pkeyutl -sign -rawin -inkey "$key" -in "$d/manifest.json" -out "$d/manifest.json.sig"
  printf %s "$(keyid "$key")" > "$d/manifest.json.keyid"
}
# publish the listed tags as GitHub's release list, served on loopback; the server's log says what was fetched
publish() { python3 -I - "$T/$CUR/feed" "$FPORT" "$@" > "$T/$CUR/feed/releases.json" <<'PY'
import json, os, sys
root, port, tags = sys.argv[1], sys.argv[2], sys.argv[3:]
print(json.dumps([{"tag_name": t, "prerelease": False, "draft": False,
    "assets": [{"name": n, "browser_download_url": f"http://127.0.0.1:{port}/{t}/{n}"} for n in sorted(os.listdir(os.path.join(root, t)))]} for t in tags]))
PY
}
served() { grep -c "GET /$1" "$T/$CUR/feed.log" 2>/dev/null || true; }

# --- a home, the feed, and a guard on build v0.0.1 (the real hub saying 0.0.1)
home() {
  H=$T/$CUR/home; B=$T/$CUR/builds; mkdir -p "$H" "$B/builds/v0.0.1" "$T/$CUR/feed"
  PORT=$((PORT+1)); FPORT=$((PORT+500)); A=${UPD_HOST:-127.0.0.1}:$PORT
  python3 -I "$ROOT/test/guard/make-home.py" "$H" --small > /dev/null || { echo "no test/guard/make-home.py (it comes with native/guard)"; exit 2; }
  printf '#!/bin/sh\nexec "%s/backplane-hub" --version-string 0.0.1 "$@"\n' "$BIN" > "$B/builds/v0.0.1/backplane-hub"; chmod +x "$B/builds/v0.0.1/backplane-hub"
  echo v0.0.1 > "$B/CURRENT"; publish
  (cd "$T/$CUR/feed" && exec python3 -I -m http.server "$FPORT" --bind 127.0.0.1) > "$T/$CUR/feed.log" 2>&1 & FP=$!
}
tok() { tr -d '[:space:]' < "$H/token"; }
guard() {
  env BACKPLANE_UPDATE_FEED="http://127.0.0.1:$FPORT/releases.json" "$BIN/backplane" hub run --home "$H" --port "$PORT" --host "${UPD_HOST:-127.0.0.1}" --builds "$B" --build v0.0.1 --log "$T/$CUR/hub.log" \
    -- --no-tailscale --foreground >> "$T/$CUR/hub.log" 2>&1 < /dev/null & GP=$!
}
st() { python3 -I -c 'import json,sys
try: v=json.load(open(sys.argv[1])); print(v.get(sys.argv[2],""))
except Exception: pass' "$H/guard/status.json" "$1"; }
# /hello, as a remote client reads it (CBOR: a map of text to text or small ints)
hello() { curl -s -m3 "http://$A/hello" | python3 -I "$HERE/cbor_hello.py"; }
says() { [ "$(hello)" = "$1 $COMMIT" ]; }
on() { [ "$(st build)" = "$1" ] && [ "$(st stable)" = True ] && says "${1#v}"; }
bad() { grep -q "^$1" "$B/BAD" 2>/dev/null; }
checked() { [ -s "$H/update/releases.json" ]; }   # the hub has asked the feed at least once
# the hourly check runs within a minute of the start; a phone's Check now or `update.check` runs one at once
up() { until_ 60 on v0.0.1; }

want() { [ "${#WANT[@]}" = 0 ] && [ "$1" != hold ] || [[ " ${WANT[*]:-} " == *" $1 "* ]]; }
WANT=("$@")
go() { CUR=$1; want "$1" || return 1; mkdir -p "$T/$CUR"; home; return 0; }

# a good release is fetched, installed on probation, proven, and answers on /hello as itself
if go good; then
  release v0.0.2 0.0.2 "$T/keys/e2e.key"; publish v0.0.2; guard
  if ! until_ 150 on v0.0.2; then no "never moved to v0.0.2 (on $(st build), /hello: $(hello))"; elif bad v0.0.2; then no "marked bad"
  elif [ ! -f "$B/builds/v0.0.2/EXPECT" ]; then no "no EXPECT"; else ok "v0.0.1 -> v0.0.2 proven; /hello says 0.0.2 $COMMIT"; fi
fi
# a release that panics in a start step comes up without bots, which the build before had: on probation that fails like a crash, goes straight back, is marked bad, and is never fetched again
if go panic; then
  release v0.0.3 0.0.3 "$T/keys/e2e.key" panic:bots; publish v0.0.3; guard
  if ! until_ 150 bad v0.0.3; then no "v0.0.3 never marked bad (on $(st build))"; elif ! grep '^v0.0.3' "$B/BAD" | grep -q bots; then no "bad, but not for the missing part: $(grep '^v0.0.3' "$B/BAD")"; elif ! until_ 60 on v0.0.1; then no "not back on v0.0.1 (/hello: $(hello))"
  else ok "panicking v0.0.3 marked bad ($(grep '^v0.0.3' "$B/BAD" | cut -f2 | cut -c1-60)), back on v0.0.1"; fi
fi
# a release whose hub dies at its start (an abort in the agents step) goes straight back, is marked bad, and is never fetched again
if go die; then
  release v0.0.6 0.0.6 "$T/keys/e2e.key" die:agents; publish v0.0.6; guard
  if ! until_ 150 bad v0.0.6; then no "v0.0.6 never marked bad (on $(st build))"; elif ! until_ 60 on v0.0.1; then no "not back on v0.0.1 (/hello: $(hello))"
  else ok "v0.0.6 dying at its start marked bad ($(grep '^v0.0.6' "$B/BAD" | cut -f2 | cut -c1-60)), back on v0.0.1"; fi
fi
# a release whose hub answers as another version than its manifest says fails the identity check
if go mismatch; then
  release v0.0.4 0.0.9 "$T/keys/e2e.key"; publish v0.0.4; guard
  if ! until_ 150 bad v0.0.4; then no "v0.0.4 never marked bad (on $(st build), /hello: $(hello))"; elif ! grep '^v0.0.4' "$B/BAD" | grep -q expected; then no "bad, but not for its identity: $(grep '^v0.0.4' "$B/BAD")"
  elif ! until_ 60 on v0.0.1; then no "not back on v0.0.1"; else ok "v0.0.4 answering as 0.0.9 marked bad ($(grep '^v0.0.4' "$B/BAD" | cut -f2 | cut -c1-70)), back on v0.0.1"; fi
fi
# an older release, properly signed, is never offered: nothing of it is fetched beyond the list
if go older; then
  release v0.0.0 0.0.0 "$T/keys/e2e.key"; publish v0.0.0; guard
  if ! up; then no "no start"; elif ! until_ 90 checked; then no "the hub never asked the feed"; else sleep 3
    if [ "$(served v0.0.0/)" != 0 ] || [ -e "$B/builds/v0.0.0" ] || [ -e "$H/guard/request" ]; then no "v0.0.0 was taken further (fetched $(served v0.0.0/))"; else ok "v0.0.0 not offered: list read, nothing of it fetched, still on v0.0.1"; fi
  fi
fi
# a release signed with a key the hub does not trust: the signed files are read, the archive never is
if go wrongkey; then
  release v0.0.5 0.0.5 "$T/keys/other.key"; publish v0.0.5; guard
  if ! up; then no "no start"; elif ! until_ 90 checked; then no "the hub never asked the feed"; else sleep 3
    if [ "$(served v0.0.5/manifest.json.sig)" = 0 ]; then no "the signature was never read"; elif [ "$(served v0.0.5/backplane-)" != 0 ] || [ -e "$B/builds/v0.0.5" ]; then no "the archive was fetched or staged"
    else ok "v0.0.5 signed by another key: signature read, archive never fetched, still on v0.0.1"; fi
  fi
fi
# the test key is held below 0.1.0 even in a hub that trusts it
if go ceiling; then
  release v0.1.0 0.1.0 "$T/keys/e2e.key"; publish v0.1.0; guard
  if ! up; then no "no start"; elif ! until_ 90 checked; then no "the hub never asked the feed"; else sleep 3
    if [ "$(served v0.1.0/backplane-)" != 0 ] || [ -e "$B/builds/v0.1.0" ]; then no "v0.1.0 under the test key was fetched"; else ok "v0.1.0 under the test key refused before its archive"; fi
  fi
fi
# what tailscale serve forwards (loopback, no credential, its headers) learns nothing of the build; the hub token does
if go healthz; then
  guard
  if ! up; then no "no start"; else
    open=$(curl -s -m3 -H "Tailscale-User-Login: someone@example.com" -H "X-Forwarded-For: 100.64.0.9" -H "X-Forwarded-Proto: https" "http://$A/healthz")
    authed=$(curl -s -m3 -H "Authorization: Bearer $(tok)" "http://$A/healthz")
    if echo "$open" | grep -q -e '"version"' -e '"commit"' -e "$COMMIT"; then no "an unauthenticated /healthz shows the build: $open"
    elif ! echo "$authed" | grep -q "\"commit\":\"$COMMIT"; then no "the hub token does not see the commit: $authed"; else ok "through serve: $(echo "$open" | cut -c1-60)…; with the token: version and commit"; fi
  fi
fi

# hold: a guarded hub on v0.0.1 for clients on other machines (test/update/clients.sh drives them). It prints `READY <dir>` once up; in <dir>, a file `publish`
# puts v0.0.2 on the feed, `stop` ends it. Never runs unless named. UPD_HOST: the address to listen on (a LAN address, so other machines reach it).
if [[ " ${WANT[*]:-} " == *" hold "* ]] && go hold; then
  release v0.0.2 0.0.2 "$T/keys/e2e.key"; guard
  if ! up; then no "no start"; else
    echo "READY $T/$CUR $A"; n=0; pub=
    while [ ! -e "$T/$CUR/stop" ] && [ $n -lt 3600 ]; do
      if [ -e "$T/$CUR/publish" ] && [ -z "$pub" ]; then publish v0.0.2; pub=1; echo "published v0.0.2"; fi
      sleep 1; n=$((n+1))
    done
    ok "held for clients ($n s)"
  fi
fi

# the phone: pair the Android app with this hub (adb reverse, so the hub stays on loopback), press Check now in Settings > Updates, see it install,
# then restart it and roll back from the Hub screen. PHONE=<serial>, APK=<debug apk of dev.backplane.mobile.gpui>; only that app is touched.
if [ -n "${PHONE:-}" ] && go phone; then
  # the phone is shared between lanes: hold its lock for this case only (released when the case's subshell ends, crash included)
  exec 9> "${PHONE_LOCK:-$HOME_REAL/.cache/pixel-$PHONE.lock}"; echo "  waiting for the phone's lock"; flock 9
  P="python3 -I $HERE/phone.py $PHONE"; PKG=dev.backplane.mobile.gpui; SH=$T/phone; mkdir -p "$SH"
  want_() { local i; for i in $(seq ${2:-20}); do $P has "$1" && return 0; sleep 1; done; $P shot "$SH/fail.png"; return 1; }
  guard
  if ! up; then no "no start"; else
    [ -z "${APK:-}" ] || adb -s "$PHONE" install -r "$APK" > /dev/null
    adb -s "$PHONE" reverse "tcp:$PORT" "tcp:$PORT" > /dev/null
    L=$(curl -s -m5 -X POST -H "Authorization: Bearer $(tok)" "http://$A/auth/invitations" | python3 -I -c 'import json,sys,urllib.parse;print(urllib.parse.quote(json.loads(sys.stdin.read(),strict=False)["link"],safe=""))')
    adb -s "$PHONE" shell pm clear $PKG > /dev/null; adb -s "$PHONE" shell pm grant $PKG android.permission.POST_NOTIFICATIONS > /dev/null 2>&1
    adb -s "$PHONE" shell am force-stop $PKG; adb -s "$PHONE" shell "am start -W -n $PKG/dev.backplane.mobile.DataActivity -d 'bpgpui://pair?url=$L'" > /dev/null
    # a link asks before it pairs ("Pair with this hub?")
    if ! { want_ "Pair with this hub?" 30 && sleep 1 && $P tap Pair exact -1; }; then no "the app did not offer to pair"
    elif ! { for _ in $(seq 30); do $P has "Pair with this hub?" || break; sleep 1; done; want_ Settings 30; }; then no "the app did not pair (no Settings on screen)"
    elif ! { $P tap Settings && want_ UPDATES && $P seek "Check now"; }; then no "no Updates section with Check now: $($P text 'Backplane' || echo '?') / $($P text 'Up' || echo '?')"
    else
      $P shot "$SH/1-updates.png"; until_ 90 checked   # the hub's own first check is over: only Check now looks again before the hour
      release v0.0.2 0.0.2 "$T/keys/e2e.key"; publish v0.0.2; $P tap "Check now"
      if ! until_ 150 on v0.0.2; then no "Check now did not install v0.0.2 (on $(st build))"
      elif ! { want_ UPDATES 60; $P seek "Updated to v0.0.2"; }; then no "the phone does not say it updated: $($P text 'Up' || $P text 'Inst' || echo '?')"
      else
        $P shot "$SH/2-updated.png"
        pid=$(st hubPid)
        if ! { adb -s "$PHONE" shell input keyevent KEYCODE_BACK; sleep 1; $P tap Settings && want_ UPDATES && $P tap Hub exact && want_ "Restart hub" 30; }; then no "the Hub screen offers no restart"
        elif ! { $P tap "Restart hub" exact && sleep 1 && $P tap "Restart hub" exact -1; }; then no "could not press Restart hub"
        elif ! until_ 90 eval '[ "$(st hubPid)" != "$pid" ] && on v0.0.2'; then no "Restart hub from the phone did not restart v0.0.2 (pid $pid -> $(st hubPid))"
        elif ! { $P shot "$SH/3-restarted.png"; want_ "Roll back to" 60; }; then no "the Hub screen offers no rollback"
        else
          $P shot "$SH/4-hub.png"; $P tap "Roll back to"; sleep 1; $P tap "Roll back to" -1   # the confirm dialog's button
          if ! until_ 120 bad v0.0.2; then no "rollback from the phone did not mark v0.0.2 bad"; elif ! until_ 90 on v0.0.1; then no "not back on v0.0.1"
          elif ! grep '^v0.0.2' "$B/BAD" | grep -q remote; then no "v0.0.2 bad, but not by a remote rollback: $(grep '^v0.0.2' "$B/BAD")"
          else $P shot "$SH/5-rolledback.png"; ok "Check now on the phone installed v0.0.2; Restart hub restarted it; Roll back went back to v0.0.1 (shots: $SH)"; fi
        fi
      fi
    fi
  fi
  adb -s "$PHONE" reverse --remove "tcp:$PORT" > /dev/null 2>&1; exec 9>&-
fi

echo "update E2E: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
