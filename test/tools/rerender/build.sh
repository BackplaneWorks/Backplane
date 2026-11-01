#!/bin/sh
# The re-render harness's hub and web client, built from this tree and
# stamped with it: <commit>+<hash of the uncommitted changes under src/ and
# scripts/>. rerender.ts records both stamps in report.json, and
# --write-budgets refuses a run whose stamps are not the tree's own, so a
# stale build never feeds the budgets.
#
#   test/tools/rerender/build.sh [OUT]     OUT defaults to build/rr
#
# Heavy (bend's emit): run it under bp-admit, after checking `df -h /`.
set -eu
cd "$(dirname "$0")/../../.."
out=${1:-build/rr}
# its own log beside its output (<out>.log), the folders made here: a fresh
# worktree needs nothing made first, and a caller redirects nothing
mkdir -p "$out"
exec 3>&1 > "$out.log" 2>&1
# what the caller sees: one line, or the log's end when it fails
trap 'status=$?; [ $status = 0 ] || { echo "build.sh: failed ($status), the end of $out.log:" >&3; tail -n 15 "$out.log" >&3; }' EXIT
echo "build.sh: $(date) into $out"
stamp=$(sh test/tools/rerender/stamp.sh)
scripts/build-js.sh src/server/main.bend "$out/backplane-serve"
printf '%s\n' "$stamp" > "$out/backplane-serve.stamp"
rm -rf "$out/web.new"
bend src/web/index.html -o "$out/web.new" >/dev/null
python3 scripts/js-unmarshal.py ../core/client.Ui "$out"/web.new/chunk-*.js
python3 scripts/js-native.py "$out"/web.new/chunk-*.js
cp src/web/sw.js "$out/web.new/sw.js"
sh scripts/build-viewer.sh "$out/web.new/viewer/vendor"
printf '%s\n' "$stamp" > "$out/web.new/BUILD-STAMP"
rm -rf "$out/web" && mv "$out/web.new" "$out/web"
# the tree must not have changed while it built
[ "$(sh test/tools/rerender/stamp.sh)" = "$stamp" ] || { echo "build.sh: the tree changed during the build" >&2; exit 1; }
echo "built $out ($stamp)"
echo "built $out ($stamp)" >&3
