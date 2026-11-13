#!/bin/bash
# bare-gate.sh: the bare-install gate (docs/bare-install-gate.md). On a fresh Boat it installs Backplane the way a person does into a clean
# container of a stock distro image and walks the journey (test/bare/journey.sh); prints the checks, the timing table and the paper cuts;
# deletes every box it made; exits non-zero on any failure. Run from a worktree on this machine; nothing of the person's goes to a box.
#
#   scripts/bare-gate.sh [--release TAG|latest] [--commit SHA] [--image ubuntu:24.04] [--keep] [--out DIR]
#     --release   install the published release (default: latest), as the docs say
#     --commit    build SHA on a build box (from the snapshot bp-build-base) into a release-shaped archive and install that (before a release)
#     --archive   a folder with an archive and install.sh build-archive.sh made already (skips the build)
#     --image     the clean machine's image (ubuntu:24.04; debian:12 and fedora:41 follow)
#     --keep      leave the gate box running for a look (you delete it: boat delete ID)
# Env: BARE_BOAT_TYPE (default large), BARE_TIMEOUT (journey seconds, 3600).
set -uo pipefail
{   # the whole script is one block: bash reads it all before running any of it, so editing this file during a run changes nothing
REPO=$(git rev-parse --show-toplevel); cd "$REPO"
. scripts/lib-bptmp.sh
SRC=release; TAG=latest; SHA=; ADIR=; IMAGE=ubuntu:24.04; KEEP=; OUT=
while [ $# -gt 0 ]; do case $1 in
  --release) SRC=release; TAG=${2:-latest}; shift;; --commit) SRC=commit; SHA=$2; shift;; --image) IMAGE=$2; shift;;
  --archive) SRC=archive; ADIR=$(realpath "$2"); shift;;
  --keep) KEEP=1;; --out) OUT=$2; shift;; *) echo "bare-gate: unknown $1" >&2; exit 2;; esac; shift; done
OUT=${OUT:-$BP_TMP/bare-gate-$(date +%Y%m%d-%H%M%S)}; mkdir -p "$OUT"
B() { boat --no-update "$@"; }
# every box this run made, and only those, are deleted at the end ($OUT/boxes)
gone() { [ -f "$OUT/boxes" ] || return 0; for id in $(sort -u "$OUT/boxes"); do case $id in bx_[a-z0-9]*) ;; *) continue;; esac   # never an empty id: `boat delete` alone deletes the current sandbox
  [ -n "$KEEP" ] && [ "$id" = "${GATE:-}" ] && { echo "kept gate box $id (boat delete $id)"; continue; }; B delete "$id" --yes >/dev/null 2>&1 && echo "deleted box $id" || echo "bare-gate: could not delete $id: boat delete $id --yes" >&2; done; }
trap gone EXIT; trap "exit 130" INT TERM HUP   # a stop runs the EXIT trap too: no box is left
newbox() { # name, extra args -> id
  local j id; j=$(B new --name "$1" --environment bp-build --no-env --no-snapshots --type "${BARE_BOAT_TYPE:-large}" --ttl 10800 --json "${@:2}" 2>&1 | tail -1)
  id=$(printf '%s' "$j" | python3 -c 'import json,sys;print(json.load(sys.stdin)["id"])' 2>/dev/null) || { echo "bare-gate: could not make a box: $j" >&2; return 1; }
  echo "$id" >>"$OUT/boxes"; echo "$id"   # called in $( ): the list is a file, read back by gone()
}
push() { tar -czf - "${@:2}" | B ssh "$1" -- 'mkdir -p ~/gate && tar -xzf - -C ~/gate'; }
FILES=(test/bare test/onboarding/pos.py test/tools/claude_standin.ts test/wire.mjs test/wire-codec.js native/host/src/wire_dict.rs test/files/fixtures/plot)
T0=$(date +%s)

ARCH_ARGS=()
if [ $SRC = commit ]; then
  full=$(git rev-parse "$SHA^{commit}") || exit 2; short=${full:0:9}
  TAGC=v0.16.0-dev.$short
  echo "bare-gate: building $short on a build box"
  BB=$(newbox "bare-build-$short" --from bp-build-base) || exit 1   # the build box: bp-build-base is for builds only, never the gate box
  git archive --format=tar "$full" | gzip | B ssh "$BB" -- "rm -rf ~/src && mkdir -p ~/src && tar -xzf - -C ~/src && echo $full > ~/src/COMMIT" || exit 1
  base=$(gh release view -R BackplaneWorks/Backplane --json tagName -q .tagName)
  B ssh "$BB" -- "cd ~ && gh release download $base -R BackplaneWorks/Backplane -p 'backplane-$base-linux-x64-native.tar.gz' -O base.tar.gz 2>/dev/null || curl -fsSL -o base.tar.gz https://github.com/BackplaneWorks/Backplane/releases/download/$base/backplane-$base-linux-x64-native.tar.gz" || exit 1
  tb=$(date +%s)
  B ssh "$BB" -- 'cat > ~/build-archive.sh' <test/bare/build-archive.sh   # this gate's builder, not the commit's (it may predate it)
  python3 scripts/boat-run.py "$BB" "bash ~/build-archive.sh ~/src ~/base.tar.gz ~/archive $TAGC" 2>&1 | tail -5 | tee "$OUT/build.log" || exit 1
  BUILD_S=$(( $(date +%s) - tb ))
  mkdir -p "$OUT/archive"; B scp "$BB:archive/backplane-$TAGC-linux-x64-native.tar.gz" "$OUT/archive/" >/dev/null && B scp "$BB:archive/install.sh" "$OUT/archive/" >/dev/null || { echo "bare-gate: no archive came back" >&2; exit 1; }
  case $BB in bx_[a-z0-9]*) B delete "$BB" --yes >/dev/null 2>&1 && { echo "deleted build box $BB"; grep -vxF "$BB" "$OUT/boxes" >"$OUT/boxes.n"; mv "$OUT/boxes.n" "$OUT/boxes"; };; esac
fi

if [ $SRC = archive ]; then
  f=$(ls "$ADIR"/backplane-v*-linux-x64-native.tar.gz | head -1); [ -f "$f" ] && [ -f "$ADIR/install.sh" ] || { echo "bare-gate: no archive and install.sh in $ADIR" >&2; exit 2; }
  TAGC=$(basename "$f" | sed 's/^backplane-\(v.*\)-linux-x64-native.tar.gz$/\1/'); mkdir -p "$OUT/archive"; cp "$f" "$ADIR/install.sh" "$OUT/archive/"; SRC=commit; TAG=
fi
GATE=$(newbox "bare-gate-$(date +%m%d-%H%M)") || exit 1
echo "bare-gate: gate box $GATE ($IMAGE, $SRC${SHA:+ $SHA}${TAG:+ $TAG})"
push "$GATE" "${FILES[@]}" || exit 1
ENV="IMAGE=$IMAGE SOURCE=release TAG=$TAG"
if [ $SRC = commit ]; then
  B scp "$OUT/archive/backplane-$TAGC-linux-x64-native.tar.gz" "$GATE:gate/" >/dev/null && B scp "$OUT/archive/install.sh" "$GATE:gate/" >/dev/null || exit 1
  ENV="IMAGE=$IMAGE SOURCE=archive ARCHIVE=\$HOME/gate/backplane-$TAGC-linux-x64-native.tar.gz INSTALLER=\$HOME/gate/install.sh"
fi
timeout "${BARE_TIMEOUT:-3600}" python3 scripts/boat-run.py "$GATE" "cd ~/gate && env $ENV bash test/bare/journey.sh ~/run" 2>&1 | tee "$OUT/journey.log"
RC=${PIPESTATUS[0]}
B ssh "$GATE" -- 'cd ~/run && tar -czf - --exclude=home --exclude=standins --exclude=projects .' | tar -xzf - -C "$OUT" 2>/dev/null

echo; echo "== timings ($IMAGE, $SRC${SHA:+ $SHA})"
[ -n "${BUILD_S:-}" ] && printf '  %-28s %8s s\n' build-on-boat "$BUILD_S"
[ -f "$OUT/timings.tsv" ] && while IFS=$'\t' read -r k v; do printf '  %-28s %8s s\n' "$k" "$v"; done <"$OUT/timings.tsv"
[ -f "$OUT/backplane-size.txt" ] && echo "  $(cat "$OUT/backplane-size.txt")"
[ -f "$OUT/kicad-size.txt" ] && echo "  $(cat "$OUT/kicad-size.txt")"
printf '  %-28s %8s s\n' gate-wall "$(( $(date +%s) - T0 ))"
echo "== paper cuts, in the order a person meets them (rank 1 = blocks or misleads)"
[ -s "$OUT/findings.tsv" ] && sort -s -t$'\t' -k1,1n "$OUT/findings.tsv" | awk -F'\t' '{printf "  [%s] %-12s %s\n", $1, $2, $3}' || echo "  none"
echo "== checks: $(grep -c ^PASS "$OUT/checks.tsv" 2>/dev/null) pass, $(grep -c ^FAIL "$OUT/checks.tsv" 2>/dev/null) fail ($OUT)"
grep ^FAIL "$OUT/checks.tsv" 2>/dev/null | sed 's/^/  /'
[ "$RC" = 0 ] && ! grep -q ^FAIL "$OUT/checks.tsv" 2>/dev/null
exit $?
}
