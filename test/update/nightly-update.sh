#!/bin/bash
# The nightly update check: a person on the nightly channel, with a project and a thread, is updated by the hub's own updater to the new nightly.
#   test/update/nightly-update.sh FROM TO
#     FROM, TO: folders each holding one published release's install.sh, linux-x64 (or this machine's) -native archive and manifest.json, .sig, .keyid,
#     as `gh release download` gives them (scripts/nightly-update-gate.sh fetches them and runs this on a clean Boat). FROM older than TO; FROM may be a
#     stable, beta or nightly; TO is signed with the real release key (nothing here signs anything).
# Steps: FROM installed by its own install.sh into a private HOME, prefix, home and port; the home seeded first with update.channel=nightly, a project
# (its folder holds a file) and a thread; FROM and TO served on a loopback feed shaped like GitHub's, a nightly's signed files under the names it is published
# with (nightly-manifest.json*, update/run.rs NIGHTLY_SIGNED); the hub's own
# first check takes TO, the guard runs it on probation and proves it. Then: /hello names TO's version and commit, CURRENT says TO, and the seeded events are
# still the head of events.jsonl (the new hub opened that store to start: a store it could not read would have failed its probation) and the project's
# file is still there. Prints PASS/FAIL lines and exits non-zero on any FAIL.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
FROM=$(cd "$1" && pwd); TO=$(cd "$2" && pwd)
tag() { python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["tag"])' "$1/manifest.json"; }
commit_of() { python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["commit"])' "$1/manifest.json"; }
T1=$(tag "$FROM"); T2=$(tag "$TO")
case "$(uname -s)-$(uname -m)" in Linux-x86_64) TGT=linux-x64;; Linux-aarch64) TGT=linux-arm64;; Darwin-arm64) TGT=darwin-arm64;; *) echo "nightly-update: no target for $(uname -sm)"; exit 2;; esac
T=$(mktemp -d "${UPD_DIR:-/tmp}/.nightly-update.XXXXXX"); P=${UPD_PORT:-3996}; FP=$((P+500)); F=$T/feed; H=$T/home; PASS=0; FAILS=()
# the feed on loopback, as GitHub lists this repo's releases
export HOME=$T/userhome BACKPLANE_PREFIX=$T/prefix BACKPLANE_BIN=$T/bin BACKPLANE_HOME=$H BACKPLANE_PORT=$P BACKPLANE_NO_DESKTOP_APP=1 BACKPLANE_NO_DESKTOP=1 \
  BACKPLANE_HUB_ARGS=--no-tailscale BACKPLANE_AGENTS=$T/agents BACKPLANE_GUARD_STABLE=10 \
  BACKPLANE_UPDATE_FEED=http://127.0.0.1:$FP/releases.json
unset BACKPLANE_COPY BACKPLANE_NO_UPDATE BACKPLANE_BUILDS BACKPLANE_GUARD BACKPLANE_E2E_TRUST_KEY BACKPLANE_AGENT_OWNER BACKPLANE_FAULT BACKPLANE_SAFE BACKPLANE_UPDATE_CHANNEL
mkdir -p "$HOME" "$T/agents" "$F" "$H" "$T/proj"
G=$BACKPLANE_PREFIX/backplane
stop() { [ -x "$G" ] && "$G" hub stop --home "$H" > /dev/null 2>&1; [ -n "${FPID:-}" ] && kill "$FPID" 2>/dev/null; }
trap 'stop; [ -n "${UPD_KEEP:-}" ] || rm -rf "${T:?}"' EXIT
ok() { PASS=$((PASS+1)); printf 'PASS %-9s %s\n' "$1" "$2"; }
no() { FAILS+=("$1"); printf 'FAIL %-9s %s\n' "$1" "$2"; tail -8 "$H/hub.log" 2>/dev/null | cut -c1-200 | sed 's/^/    /'; }
until_() { local n=$1; shift; local i=0; while [ "$i" -lt $((n*2)) ]; do if "$@"; then return 0; fi; sleep 0.5; i=$((i+1)); done; return 1; }
st() { python3 -I -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get(sys.argv[2],""))
except Exception: pass' "$H/guard/status.json" "$1"; }
hello() { curl -s -m3 "http://127.0.0.1:$P/hello" | python3 -I "$HERE/cbor_hello.py"; }
says() { local h; h=$(hello); [ "${h%% *}" = "${2#v}" ] && case "$(commit_of "$1")" in "${h#* }"*) true;; *) false;; esac; }
on() { [ "$(st build)" = "$2" ] && [ "$(st stable)" = True ] && says "$1" "$2"; }
listing() { # name dir... : a GitHub-shaped release list of the given releases
  local out=$1; shift
  for d in "$@"; do t=$(tag "$d"); mkdir -p "$F/$t"; cp "$d"/backplane-"$t"-$TGT-native.* "$F/$t/"
    case $t in *-nightly.*) for f in manifest.json manifest.json.sig manifest.json.keyid; do cp "$d/$f" "$F/$t/nightly-$f"; done;; *) cp "$d"/manifest.json* "$F/$t/";; esac; done
  python3 -I - "$F" "$FP" "$@" > "$F/$out" <<'PY'
import json, os, sys
f, port = sys.argv[1], sys.argv[2]
tags = [json.load(open(os.path.join(d, "manifest.json")))["tag"] for d in sys.argv[3:]]
print(json.dumps([{"tag_name": t, "prerelease": "-" in t, "draft": False, "assets": [{"name": n, "browser_download_url": f"http://127.0.0.1:{port}/{t}/{n}"} for n in sorted(os.listdir(os.path.join(f, t)))]} for t in tags]))
PY
}
listing releases.json "$FROM" "$TO"
(cd "$F" && exec python3 -I -m http.server "$FP" --bind 127.0.0.1) > "$T/feed.log" 2>&1 & FPID=$!

# a person's data, set before the first start: the nightly channel, a project whose folder holds work, a thread in it
echo "work that must survive the update" > "$T/proj/notes.txt"
cat > "$H/events.jsonl" <<EOF
{"\$":"SettingSet","key":"update.channel","value":"nightly","at":1}
{"\$":"ProjectCreated","id":"pnightly","title":"Nightly check","root":"$T/proj","at":2}
{"\$":"ThreadCreated","id":"tnightly","project":"pnightly","title":"Kept across the update","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3}
EOF
cp "$H/events.jsonl" "$T/seed.jsonl"

# 1. FROM installed as a person installs it
BACKPLANE_ARCHIVE="$FROM/backplane-$T1-$TGT-native.tar.gz" sh "$FROM/install.sh" --quiet > "$T/install.log" 2>&1 < /dev/null; rc=$?
if [ $rc != 0 ]; then no install "$T1's install.sh failed ($rc): $(tail -2 "$T/install.log")"
elif ! until_ 120 eval '[ "$(st build)" = "$T1" ] && says "$FROM" "$T1"'; then no install "the guard did not bring up $T1 (state $(st state), /hello $(hello))"
else ok install "$T1 installed by its install.sh; /hello $(hello)"; fi

# 2. the hub's own updater, on the nightly channel, takes TO from the nightly feed; the guard proves it
if [ ${#FAILS[@]} = 0 ]; then
  if ! until_ 300 on "$TO" "$T2"; then no update "not on $T2 (on $(st build), state $(st state), BAD: $(cat "$BACKPLANE_PREFIX/BAD" 2>/dev/null))"
  elif [ "$(tr -d '\r' < "$BACKPLANE_PREFIX/CURRENT")" != "$T2" ]; then no update "proven, but CURRENT says $(cat "$BACKPLANE_PREFIX/CURRENT")"
  else ok update "$T1 -> $T2 by the hub's updater on the nightly channel: verified against release.pub, staged, proven; /hello $(hello)"; fi
fi

# 3. the data: the seeded events still head the store the new hub runs on, and the project's folder is untouched
if [ ${#FAILS[@]} = 0 ]; then
  n=$(wc -l < "$T/seed.jsonl")
  if ! head -n "$n" "$H/events.jsonl" | cmp -s - "$T/seed.jsonl"; then no data "the seeded events are not the head of events.jsonl any more"
  elif [ "$(cat "$T/proj/notes.txt" 2>/dev/null)" != "work that must survive the update" ]; then no data "the project's file changed or went"
  elif ! until_ 30 on "$TO" "$T2"; then no data "$T2 did not stay up on this home"
  else ok data "project, thread and channel kept: events.jsonl $(wc -l < "$H/events.jsonl") lines, its first $n as seeded; $T2 runs on them"; fi
fi

echo "nightly update: $PASS passed, ${#FAILS[@]} failed${FAILS:+: ${FAILS[*]}}"; [ ${#FAILS[@]} = 0 ]
