#!/bin/bash
# Every Linux E2E row on one commit, one command, one summary to paste.
#   scripts/e2e-linux.sh [--no-build] [--no-phone] [OUT_DIR]       (run it in a checkout of the commit to judge)
#   scripts/e2e-linux.sh --archive backplane-vX.Y.Z-linux-x64-native.tar.gz [--no-phone] [OUT_DIR]
#       the same rows against a RELEASE ARCHIVE instead of a build: install.sh (filled in from deploy/install.sh.in with the archive's sha256) installs it into a private
#       prefix, home and port with the guard running its hub; the rows then use the installed hub, client, assets, CEF runtime and browser helper, so a file the archive
#       lacks or a path only a dev build has fails here. The summary's heading says "release archive vX". The archive is never published or uploaded.
# Builds that checkout (scripts/dev-build.sh --no-cut, its own GPUI_DEV_DIR; nothing is published), then runs on private hubs with stand-in agents, in this order:
#   hub rows       test/e2e/linux-f-rows.mjs      F05-F22 and S04, through the websocket
#   settings walk  test/e2e/settings-walk.mjs     every control on every Settings page, applied and persisted over a restart
#   client rows    test/e2e/gpui-f-rows.py        the GPUI window on a private Xvfb + i3 with real X input
#   CEF            test/cef/cef_e2e.sh            the embedded Chromium in the window
#   CEF lifecycle  test/cef/lifecycle_e2e.sh      https and file:// pages, back, typing, scrolling, the tab closed/reopened and quit with no CEF helper left
#   phone rows     test/e2e/pixel-f-rows.sh       the Pixel 9; the script takes ~/.cache/pixel-52220DLAQ001FC.lock itself and holds it for its whole run (skipped with --no-phone, or when no device is attached)
# OUT_DIR/summary.md is the paste: the commit, what ran, counts per driver, every failure with its evidence. Exit 0 only when nothing failed.
# Frame budgets are not judged: llvmpipe is software rendering. A row that was not run says why (skip), never pass.
set -u
BUILD=1; PHONE=1; OUT=; ARCHIVE=
while [ $# -gt 0 ]; do case "$1" in --no-build) BUILD=0;; --no-phone) PHONE=0;; --archive) shift; ARCHIVE=$(realpath "$1"); BUILD=0;; *) OUT=$1;; esac; shift; done
ROOT=$(cd "$(dirname "$0")/.." && pwd); cd "$ROOT"
SHA=$(git rev-parse HEAD); SHORT=$(git rev-parse --short HEAD); DIRTY=$(git status --porcelain --untracked-files=no | wc -l)
LABEL="$SHORT"; [ -n "$ARCHIVE" ] && { TAG=$(basename "$ARCHIVE" | sed -E 's/^backplane-(v.*)-linux-.*/\1/'); LABEL="release-$TAG"; }
OUT=${OUT:-$HOME/.cache/e2e-linux-$LABEL}; mkdir -p "$OUT/tmp"; OUT=$(cd "$OUT" && pwd)
: > "$OUT/results.tsv"   # driver<TAB>status<TAB>id<TAB>evidence
add() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$OUT/results.tsv"; }
export TMPDIR=$OUT/tmp NO_SCOPE=1 PERF_REPO=$ROOT BACKPLANE_NO_UPDATE=1; unset CLAUDE_CONFIG_DIR
HUB=$ROOT/native/host/target/fast/backplane-hub; GUI=$ROOT/native/gpui/target/fast/backplane-gpui; ASSETS=$ROOT/assets
if [ $BUILD = 1 ]; then
  echo "== build $SHORT"; GPUI_DEV_DIR=$OUT/dev NO_SCCACHE=1 scripts/dev-build.sh --no-cut > "$OUT/build.log" 2>&1 || { echo "build failed: $OUT/build.log"; tail -5 "$OUT/build.log"; exit 2; }
fi
PRE=()   # steps that ran before the rows, for the summary
if [ -n "$ARCHIVE" ]; then
  echo "== install $(basename "$ARCHIVE")"
  PFX=$OUT/prefix; GH=$OUT/guardhome; UH=$OUT/userhome; GP=${ARCHIVE_PORT:-3995}; rm -rf "$PFX" "$GH" "$UH"; mkdir -p "$UH"
  SUMX=$(sha256sum "$ARCHIVE" | cut -d' ' -f1); INST=$OUT/install.sh
  sed -e "s|@REPO@|BackplaneWorks/Backplane|" -e "s|@VERSION@|$TAG|" -e "s|@SHA_LINUX_X64@|$SUMX|" -e "s|@SHA_LINUX_ARM64@|$SUMX|" -e "s|@SHA_DARWIN_ARM64@||" deploy/install.sh.in > "$INST"
  ( export HOME=$UH BACKPLANE_PREFIX=$PFX BACKPLANE_BIN=$OUT/bin BACKPLANE_HOME=$GH BACKPLANE_PORT=$GP BACKPLANE_START=1 BACKPLANE_NO_DESKTOP=1 BACKPLANE_HUB_ARGS=--no-tailscale BACKPLANE_ARCHIVE=$ARCHIVE BACKPLANE_NO_UPDATE=1 BACKPLANE_AGENTS=$OUT/agents-none
    mkdir -p "$OUT/agents-none"; unset BACKPLANE_COPY BACKPLANE_BUILDS BACKPLANE_GUARD; sh "$INST" --quiet > "$OUT/install.log" 2>&1 ) || { echo "install failed: $OUT/install.log"; tail -5 "$OUT/install.log"; exit 2; }
  B=$PFX/builds/$TAG; HUB=$B/backplane-hub; GUI=$B/backplane-gpui; ASSETS=$B/assets
  for i in $(seq 1 60); do curl -s -m2 -o /dev/null "http://127.0.0.1:$GP/hello" && break; sleep 1; done
  # what the guard's own hub says (a CBOR hello): the version the archive claims
  HELLO=$(curl -s -m3 "http://127.0.0.1:$GP/hello" | strings | tr '\n' ' ' | cut -c1-80)
  if curl -s -m3 -o /dev/null "http://127.0.0.1:$GP/hello"; then add install ok guard "installed into a private prefix with install.sh, the guard started its hub on port $GP: hello says: $HELLO"; else add install FAIL guard "the guard's hub did not answer /hello on port $GP (see install/guardhome/guard.log)"; fi
  # the archive's files, by name: what a dev build has beside it and a release must carry
  for f in backplane-hub backplane-gpui backplane backplane-browser backplane-step2glb backplane-voice libcef.so icudtl.dat resources.pak v8_context_snapshot.bin COMMIT; do [ -e "$B/$f" ] || add install FAIL "file:$f" "the archive has no $f"; done
  [ -d "$B/assets" ] && [ -d "$B/locales" ] || add install FAIL "folders" "the archive has no assets/ or locales/"
  export BACKPLANE_CEF=$B E2E_ASSETS=$B/assets E2E_BROWSER=$B/backplane-browser LD_LIBRARY_PATH=$B
  "$PFX/backplane" hub stop --home "$GH" > /dev/null 2>&1   # the rows start their own hubs from the installed binary; the guard's is done
fi
[ -x "$HUB" ] && [ -x "$GUI" ] || { echo "no binaries at $HUB / $GUI (build first)"; exit 2; }
export LD_LIBRARY_PATH=$(dirname "$GUI")${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
SUM=$OUT/summary.md
parse() { # driver logfile: lines "ok   ID evidence" / "FAIL ID evidence" / "skip ID evidence" (hub, client and phone rows)
  local st id ev l; while IFS= read -r l; do
    case "$l" in ok\ *|FAIL\ *|skip\ *) read -r st id ev <<< "$l"; add "$1" "$st" "$id" "${ev//$'\t'/ }";; esac
  done < "$2"; }
run() { # name command...
  local n=$1; shift; echo "== $n"; "$@" > "$OUT/$n.log" 2>&1; local rc=$?; echo "   exit $rc"; return $rc; }
run hub-rows bun test/e2e/linux-f-rows.mjs "$HUB" "$OUT/hub-rows" 3881; parse hub-rows "$OUT/hub-rows.log"
run settings-walk bun test/e2e/settings-walk.mjs "$HUB" "$OUT/walk" 3830
python3 - "$OUT/walk/settings-walk.json" "$OUT/results.tsv" <<'PY'
import json,sys
try: rows=json.load(open(sys.argv[1]))
except Exception as e: open(sys.argv[2],'a').write("settings-walk\tFAIL\twalk\tno result file: %s\n"%e); sys.exit()
ctl=[r for r in rows if r.get('control') in ('chip','form')]
bad=[r for r in ctl if not r.get('applied')]
per=[r for r in rows if r.get('persist')]; lost=[r for r in per if not r.get('lit_after_restart')]
f=open(sys.argv[2],'a')
f.write("settings-walk\t%s\twalk\t%d controls walked on %d pages, %d not applied; %d of them re-read after a hub restart, %d lost their value. Effect proofs are S01-S14's, not this walk's\n"%('FAIL' if bad or lost else 'ok',len(ctl),len({r.get('page') for r in ctl}),len(bad),len(per),len(lost)))
for r in (bad+lost)[:20]: f.write("settings-walk\tFAIL\t%s/%s\t%s\n"%(r.get('page'),r.get('label') or r.get('key'),json.dumps(r)[:200]))
PY
if [ -d "$ASSETS" ]; then run client-rows python3 -W ignore test/e2e/gpui-f-rows.py "$GUI" "$HUB" "$ASSETS" "$OUT/client" 3891 77; parse client-rows "$OUT/client-rows.log"; else add client-rows skip all "no assets folder at $ASSETS"; fi
run cef env OUT="$OUT/cef" DISP=:78 E2E_ASSETS="${E2E_ASSETS:-$ASSETS}" sh test/cef/cef_e2e.sh "$GUI" "$HUB"
python3 - "$OUT/cef.log" "$OUT/results.tsv" <<'PY'
import sys,re
f=open(sys.argv[2],'a')
for l in open(sys.argv[1],errors='replace'):
    m=re.match(r'^(PASS|FAIL) (.*)$',l.rstrip())
    if m: f.write("cef\t%s\tcef\t%s\n"%('ok' if m.group(1)=='PASS' else 'FAIL',m.group(2)[:200]))
PY
run cef-link env HPORT=3893 BACKPLANE_CEF="${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}" sh test/cef/link_only_e2e.sh "$GUI" "$HUB"
python3 - "$OUT/cef-link.log" "$OUT/results.tsv" <<'PY'
import sys,re
f=open(sys.argv[2],'a')
for l in open(sys.argv[1],errors='replace'):
    m=re.match(r'^(PASS|FAIL) (.*)$',l.rstrip())
    if m: f.write("cef-link\t%s\tcef-link\t%s\n"%('ok' if m.group(1)=='PASS' else 'FAIL',m.group(2)[:200]))
PY
run cef-life env OUT="$OUT/cef-life" DISP=:79 E2E_ASSETS="${E2E_ASSETS:-$ASSETS}" BACKPLANE_CEF="${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}" sh test/cef/lifecycle_e2e.sh "$GUI" "$HUB"
python3 - "$OUT/cef-life.log" "$OUT/results.tsv" <<'PY'
import sys,re
f=open(sys.argv[2],'a')
for l in open(sys.argv[1],errors='replace'):
    m=re.match(r'^(PASS|FAIL) (.*)$',l.rstrip())
    if m: f.write("cef-life\t%s\tcef-life\t%s\n"%('ok' if m.group(1)=='PASS' else 'FAIL',m.group(2)[:200]))
PY
if [ $PHONE = 1 ] && adb devices 2>/dev/null | grep -q "^52220DLAQ001FC.device"; then
  echo "== phone rows (waiting for the lock)"; test/e2e/pixel-f-rows.sh "$HUB" "$OUT/phone" > "$OUT/phone-rows.log" 2>&1; parse phone-rows "$OUT/phone-rows.log"
elif [ $PHONE = 0 ]; then add phone-rows skip all "not run: --no-phone"; else add phone-rows skip all "not run: the Pixel 9 (52220DLAQ001FC) is not attached"; fi
python3 - "$OUT" "$SHA" "$DIRTY" "$LABEL" "${ARCHIVE:+$(sha256sum "$ARCHIVE" | cut -c1-12)}" <<'PY'
import sys,collections,datetime,subprocess
out,sha,dirty,label,asha=sys.argv[1:6]
rows=[l.rstrip('\n').split('\t',3) for l in open(out+'/results.tsv') if l.strip()]
by=collections.OrderedDict()
for d,s,i,e in rows: by.setdefault(d,[]).append((s,i,e))
host=subprocess.run(['uname','-sr'],capture_output=True,text=True).stdout.strip()
head=("release archive %s (sha256 %s…, driver scripts from `%s`)"%(label.replace('release-',''),asha,sha[:10])) if asha else ("`%s`%s"%(sha[:10],' (tree has uncommitted changes)' if dirty!='0' else ''))
L=["## Linux E2E on "+head,"",
   "Run %s UTC on this desktop (%s), private hubs with stand-in agents, GPUI client on a private Xvfb with i3 and real X input, llvmpipe (frame budgets are not judged)."%(datetime.datetime.utcnow().strftime('%Y-%m-%d %H:%M'),host),"",
   "| driver | ok | fail | skipped |","|---|---|---|---|"]
tot=0
for d,rs in by.items():
    n=lambda k:sum(1 for r in rs if r[0]==k); tot+=n('FAIL'); L.append("| %s | %d | %d | %d |"%(d,n('ok'),n('FAIL'),n('skip')))
L.append("")
for d,rs in by.items():
    for s,i,e in rs:
        if s in('FAIL','skip'): L.append("- **%s** `%s` %s: %s"%(d,i,'FAILED' if s=='FAIL' else 'not run',e[:300]))
L.append("")
L.append("**%s**"%('All rows that ran passed.' if tot==0 else '%d row(s) failed.'%tot))
open(out+'/summary.md','w').write('\n'.join(L)+'\n'); print('\n'.join(L)); sys.exit(1 if tot else 0)
PY
