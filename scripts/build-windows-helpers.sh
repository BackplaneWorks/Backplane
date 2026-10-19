#!/bin/bash
# Native Windows Bun helpers; run on Linux with BUN pointing to Bun 1.4.3+.
set -euo pipefail
cd "$(dirname "$0")/.."
BUN=${BUN:-bun}
out=$PWD/build/windows-helpers
mkdir -p "$out/licenses/step2glb" "$out/licenses/kipc" "$out/licenses/bun"
bun_version=$("$BUN" --version)
[[ $bun_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Unsupported Bun version: $bun_version" >&2; exit 1; }
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/oven-sh/bun/bun-v$bun_version/LICENSE.md" -o "$out/licenses/bun/LICENSE.md"
printf '%s\n' "Bun $bun_version source: https://github.com/oven-sh/bun/tree/bun-v$bun_version" > "$out/licenses/bun/SOURCE.txt"
(cd tools/step2glb && "$BUN" install --frozen-lockfile)
"$BUN" build tools/step2glb/step2glb.ts --compile --target=bun-windows-x64 --minify --outfile "$out/backplane-step2glb.exe"
cp tools/step2glb/node_modules/occt-import-js/dist/license.occt.txt tools/step2glb/node_modules/occt-import-js/dist/license.occt-import-js.txt "$out/licenses/step2glb/"
(cd tools/kipc && "$BUN" install --frozen-lockfile && BUN_TARGET=bun-windows-x64 "$BUN" build.ts "$out/backplane-kipc.exe")
python3 - <<'LICENSES'
from pathlib import Path
import hashlib, json, shutil
root=Path.cwd()
out=root/'build/windows-helpers'
modules=root/'tools/kipc/node_modules'
licenses=list(modules.rglob('LICENSE*'))
if not licenses: raise SystemExit('KiCad helper dependency licenses missing')
for p in licenses:
    if p.is_file():
        target=out/'licenses/kipc'/p.relative_to(modules)
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(p,target)
for name in ['backplane-kipc.exe','backplane-step2glb.exe']:
    if (out/name).read_bytes()[:2] != b'MZ': raise SystemExit('Not a Windows PE: '+name)
dependencies={name:json.loads((root/path/'package.json').read_text())['version']
              for name,path in [('occt-import-js','tools/step2glb/node_modules/occt-import-js'),
                                ('protobufjs','tools/kipc/node_modules/protobufjs')]}
(out/'BUILD.json').write_text(json.dumps({'dependencies':dependencies},indent=2)+'\n')
(out/'licenses/step2glb/SOURCE.txt').write_text(
    'occt-import-js '+dependencies['occt-import-js']+' source and build instructions:\n'
    'https://github.com/kovacsv/occt-import-js/tree/c2148e54b456b571238d35cac037d304053d64b2\n'
    'Original npm package: https://registry.npmjs.org/occt-import-js/-/occt-import-js-'+dependencies['occt-import-js']+'.tgz\n')
manifest={p.relative_to(out).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
          for p in sorted(out.rglob('*')) if p.is_file() and p.name!='SHA256SUMS.json'}
(out/'SHA256SUMS.json').write_text(json.dumps(manifest,indent=2)+'\n')
LICENSES
