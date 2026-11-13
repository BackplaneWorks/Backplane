#!/bin/bash
# nightly-update-gate.sh: the update check (test/update/nightly-update.sh) on a clean Boat box: a release installed as a person installs it, then updated
# by the hub's own updater on the nightly channel to a newer signed build, keeping its project and thread. Every box it makes is deleted at the end.
#   scripts/nightly-update-gate.sh [--from SRC] [--to SRC] [--out DIR]
#     SRC: a tag of BackplaneWorks/Backplane (a release, beta or nightly), `run:ID` (a release.yml dry run's release-dry-run
#     artifact: signed with the real key), or a folder already holding one release's files.
#     Defaults: --to the newest nightly, --from the one before it (the newest stable once that is a version with the nightly channel, 0.15.4 on).
# Env: NU_BOAT_TYPE (default), NU_TIMEOUT (s, 1800).
{
set -u
REPO=$(git rev-parse --show-toplevel); cd "$REPO"
. scripts/lib-bptmp.sh
MREPO=BackplaneWorks/Backplane
FROM=; TO=; OUT=
while [ $# -gt 0 ]; do case $1 in --from) FROM=$2; shift;; --to) TO=$2; shift;; --out) OUT=$2; shift;; *) echo "nightly-update-gate: unknown $1" >&2; exit 2;; esac; shift; done
OUT=${OUT:-$BP_TMP/nightly-update-$(date +%Y%m%d-%H%M%S)}; mkdir -p "$OUT"
B() { boat --no-update "$@"; }
nightlies() { gh release list -R "$MREPO" --limit 60 --json tagName -q '.[].tagName' 2>/dev/null | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+-nightly\.[0-9]+\.[0-9]+$' | sort -V -r; }
if [ -z "$TO" ]; then TO=$(nightlies | sed -n 1p); [ -n "$TO" ] || { echo "nightly-update-gate: no nightly in $MREPO yet; give --to" >&2; exit 2; }; fi
if [ -z "$FROM" ]; then FROM=$(nightlies | grep -vxF "$TO" | sed -n 1p); [ -n "$FROM" ] || { echo "nightly-update-gate: no nightly before $TO; give --from" >&2; exit 2; }; fi
fetch() { # SRC dir: one release's files for linux-x64
  local s=$1 d=$2; mkdir -p "$d"
  case $s in
    run:*) gh run download "${s#run:}" -R "$MREPO" -n release-dry-run -D "$d" >/dev/null || return 1;;
    v*-nightly.*) gh release download "$s" -R "$MREPO" -D "$d" -p install.sh -p 'nightly-manifest.json*' -p "backplane-$s-linux-x64-native.tar.gz" || return 1
      for f in manifest.json manifest.json.sig manifest.json.keyid; do mv "$d/nightly-$f" "$d/$f" || return 1; done;;   # as the test takes them (it serves them under these names again)
    v*) gh release download "$s" -R "$MREPO" -D "$d" -p install.sh -p 'manifest.json*' -p "backplane-$s-linux-x64-native.tar.gz" || return 1;;
    *) [ -d "$s" ] && cp "$s"/install.sh "$s"/manifest.json* "$s"/backplane-v*-linux-x64-native.tar.gz "$d/" || return 1;;
  esac
  [ -s "$d/manifest.json" ] && [ -s "$d/manifest.json.sig" ] && [ -s "$d/install.sh" ] && ls "$d"/backplane-v*-linux-x64-native.tar.gz >/dev/null 2>&1
}
fetch "$FROM" "$OUT/from" || { echo "nightly-update-gate: could not get $FROM" >&2; exit 1; }
fetch "$TO" "$OUT/to" || { echo "nightly-update-gate: could not get $TO" >&2; exit 1; }
# only what the box needs: the two releases' linux-x64 files
for d in from to; do find "$OUT/$d" -maxdepth 1 -type f ! -name install.sh ! -name 'manifest.json*' ! -name 'backplane-v*-linux-x64-native.tar.gz' -delete; done
echo "nightly-update-gate: $FROM -> $TO"
BOX=
gone() { case ${BOX:-} in bx_[a-z0-9]*) B delete "$BOX" --yes >/dev/null 2>&1 && echo "deleted box $BOX" || echo "nightly-update-gate: could not delete $BOX: boat delete $BOX --yes" >&2;; esac; }   # never an empty id
trap gone EXIT; trap "exit 130" INT TERM HUP
j=$(B new --name "nightly-update-$(date +%m%d-%H%M)" --environment bp-build --no-env --no-snapshots --type "${NU_BOAT_TYPE:-default}" --ttl 5400 --json 2>&1 | tail -1)
BOX=$(printf '%s' "$j" | python3 -c 'import json,sys;print(json.load(sys.stdin)["id"])' 2>/dev/null) || { echo "nightly-update-gate: could not make a box: $j" >&2; exit 1; }
echo "nightly-update-gate: clean box $BOX"
tar -czf - test/update/nightly-update.sh test/update/cbor_hello.py -C "$OUT" from to | B ssh "$BOX" -- 'rm -rf ~/nu && mkdir -p ~/nu && tar -xzf - -C ~/nu' || exit 1
timeout "${NU_TIMEOUT:-1800}" python3 scripts/boat-run.py "$BOX" "cd ~/nu && UPD_DIR=\$HOME bash test/update/nightly-update.sh from to" 2>&1 | tee "$OUT/update.log"
RC=${PIPESTATUS[0]}
echo "== nightly update $FROM -> $TO: $([ "$RC" = 0 ] && echo passed || echo "FAILED (rc $RC)") ($OUT)"
exit "$RC"
}
