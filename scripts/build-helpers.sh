#!/bin/sh
# The helper programs the hub runs (src/server/kicad.bend's Helper.find:
# backplane-kipc, -step2glb, -voice, -browser) for one target, into DIR,
# beside where the service goes, with their notices in DIR/licenses/helpers.
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

(cd tools/kipc && "$BUN" install --frozen-lockfile >/dev/null && "$BUN" build.ts "$out/backplane-kipc")
(cd tools/step2glb && "$BUN" install --frozen-lockfile >/dev/null)
"$BUN" build tools/step2glb/step2glb.ts --compile --minify \
  ${BUN_TARGET:+--target="$BUN_TARGET"} ${BUN_EXECUTABLE:+--compile-executable-path="$BUN_EXECUTABLE"} --outfile "$out/backplane-step2glb"
(cd tools/voice && "$BUN" build.ts "$out/backplane-voice")
(cd tools/browser && "$BUN" install --frozen-lockfile >/dev/null && "$BUN" build.ts "$out/backplane-browser")
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
python3 scripts/check-helpers.py "$out" --target "${target:-host}" --service-dir
