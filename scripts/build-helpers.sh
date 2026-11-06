#!/bin/sh
# The helper programs the hub runs (src/server/kicad.bend's Helper.find:
# backplane-kipc, -step2glb, -voice, -browser) for one target, into DIR,
# beside where the service goes, with their notices in DIR/licenses/helpers,
# and KiStack's skills as DIR/kistack (scripts/kistack.py, the pinned copy).
# A release builds them with the service (scripts/service-release.py js), so
# the desktop app's backplane-service folder carries them: 0.13.0 shipped
# none on macOS and Linux, and KiCad IPC never started (issue #510).
#   scripts/build-helpers.sh DIR
# BACKPLANE_JS_TARGET (linux-x64, linux-arm64, darwin-x64, darwin-arm64)
# cross-compiles; BACKPLANE_JS_BUN_RUNTIME is that target's pinned Bun
# runtime (else bun fetches its own). Windows' come from its pinned runtime
# bundle (scripts/service-runtime.py) or scripts/build-windows-helpers.sh.
set -eu
cd "$(dirname "$0")/.."
root=$PWD
out=${1:?build-helpers.sh DIR}
mkdir -p "$out"
out=$(cd "$out" && pwd)
BUN=${BUN:-$(command -v bun || echo "$HOME/.bun/bin/bun")}
target=${BACKPLANE_JS_TARGET:-}
case $target in
  "") ;;
  linux-x64|linux-arm64|darwin-x64|darwin-arm64) BUN_TARGET=bun-$target; export BUN_TARGET ;;
  windows-*) echo "build-helpers.sh: Windows' helpers come from scripts/build-windows-helpers.sh" >&2; exit 1 ;;
  *) echo "build-helpers.sh: BACKPLANE_JS_TARGET=$target" >&2; exit 1 ;;
esac
if [ -n "${BACKPLANE_JS_BUN_RUNTIME:-}" ]; then
  BUN_EXECUTABLE=$BACKPLANE_JS_BUN_RUNTIME; export BUN_EXECUTABLE
fi
lic=$out/licenses/helpers
mkdir -p "$lic/kipc" "$lic/step2glb" "$lic/browser"

# The installs stay serial (they share bun's global cache); the four
# compiles are independent (each writes only its own $out/backplane-*),
# so they run side by side. BACKPLANE_HELPER_JOBS=1 is the old serial
# order. Output of a parallel compile goes to its own log and is printed
# in the fixed kipc/step2glb/voice/browser order once all four finish.
(cd tools/kipc && "$BUN" install --frozen-lockfile >/dev/null)
(cd tools/step2glb && "$BUN" install --frozen-lockfile >/dev/null)
(cd tools/browser && "$BUN" install --frozen-lockfile >/dev/null)
compile_kipc() { (cd tools/kipc && "$BUN" build.ts "$out/backplane-kipc"); }
compile_step2glb() {
  "$BUN" build tools/step2glb/step2glb.ts --compile --minify \
    ${BUN_TARGET:+--target="$BUN_TARGET"} ${BUN_EXECUTABLE:+--compile-executable-path="$BUN_EXECUTABLE"} --outfile "$out/backplane-step2glb"
}
compile_voice() { (cd tools/voice && "$BUN" build.ts "$out/backplane-voice"); }
compile_browser() { (cd tools/browser && "$BUN" build.ts "$out/backplane-browser"); }
jobs=${BACKPLANE_HELPER_JOBS:-4}
case $jobs in ''|*[!0-9]*|0) echo "build-helpers.sh: BACKPLANE_HELPER_JOBS=$jobs" >&2; exit 1 ;; esac
# The width is capped by this process's own cgroup memory.max (read from
# the kernel, never an env var): each compile gets a 800 MB allowance, so
# under a 3.7 GB cap at most 4 run at once and a smaller cap runs fewer.
# "max" (no cgroup limit) keeps the requested width; a limit that cannot
# be read at all falls back to the serial order. An explicit
# BACKPLANE_HELPER_JOBS is still capped by a readable finite limit.
if [ "$jobs" -gt 1 ]; then
  cg=$(cut -d: -f3 /proc/self/cgroup 2>/dev/null || true)
  memmax=""
  for f in "/sys/fs/cgroup$cg/memory.max" /sys/fs/cgroup/memory.max; do
    [ -n "$cg" ] && [ -r "$f" ] && { memmax=$(cat "$f" 2>/dev/null || true); break; }
  done
  case $memmax in
    max) ;;
    ''|*[!0-9]*) echo "build-helpers.sh: cgroup memory.max unreadable; building helpers serially" >&2; jobs=1 ;;
    *)
      width=$(( memmax / 800000000 ))
      if [ "$width" -lt 1 ]; then width=1; fi
      if [ "$width" -lt "$jobs" ]; then
        echo "build-helpers.sh: cgroup memory.max=$memmax caps helper compiles at $width at a time"
        jobs=$width
      fi ;;
  esac
fi
if [ "$jobs" = 1 ]; then
  compile_kipc
  compile_step2glb
  compile_voice
  compile_browser
else
  hwork=$(mktemp -d)
  trap 'rm -rf "$hwork"' 0 1 2 15
  # at most $jobs compiles at a time, in waves of the fixed order
  hpids="" hnames="" hn=0 hfail=0
  hflush() {
    hi=1
    for pid in $hpids; do
      name=$(echo $hnames | cut -d' ' -f$hi) rc=0
      wait "$pid" || rc=$?
      cat "$hwork/$name.log"
      if [ "$rc" != 0 ]; then echo "build-helpers.sh: $name did not build (status $rc)" >&2; hfail=1; fi
      hi=$(( hi + 1 ))
    done
    hpids="" hnames="" hn=0
  }
  for name in kipc step2glb voice browser; do
    compile_$name >"$hwork/$name.log" 2>&1 &
    hpids="$hpids $!" hnames="$hnames $name" hn=$(( hn + 1 ))
    if [ "$hn" -ge "$jobs" ]; then hflush; fi
  done
  hflush
  [ "$hfail" = 0 ] || exit 1
  rm -rf "$hwork"; trap - 0 1 2 15
fi
chmod 755 "$out/backplane-kipc" "$out/backplane-step2glb" "$out/backplane-voice" "$out/backplane-browser"

# the notices: OpenCascade's and occt-import-js's (LGPL-2.1, embedded in
# step2glb as WASM) with where their source is, and every licence and
# notice file of the packages bundled into kipc and browser
cp tools/step2glb/node_modules/occt-import-js/dist/license.occt.txt \
   tools/step2glb/node_modules/occt-import-js/dist/license.occt-import-js.txt "$lic/step2glb/"
python3 - "$root" "$lic" <<'PY'
import json, shutil, sys
from pathlib import Path
root, lic = Path(sys.argv[1]), Path(sys.argv[2])
occt = json.loads((root/'tools/step2glb/node_modules/occt-import-js/package.json').read_text())['version']
(lic/'step2glb/SOURCE.txt').write_text(
    'occt-import-js '+occt+' source and build instructions:\n'
    'https://github.com/kovacsv/occt-import-js/tree/c2148e54b456b571238d35cac037d304053d64b2\n'
    'Original npm package: https://registry.npmjs.org/occt-import-js/-/occt-import-js-'+occt+'.tgz\n')
for tool in ('kipc', 'browser'):
    modules = root/'tools'/tool/'node_modules'
    found = [p for p in modules.rglob('*') if p.is_file() and not p.is_symlink()
             and p.name.upper().startswith(('LICENSE', 'LICENCE', 'NOTICE', 'COPYING'))]
    if not found: raise SystemExit('build-helpers.sh: no licences under tools/'+tool+'/node_modules')
    for p in found:
        dest = lic/tool/p.relative_to(modules)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(p, dest)
PY
scripts/build-kistack.sh "$out/kistack"
python3 scripts/check-helpers.py "$out" --target "${target:-host}" --service-dir
