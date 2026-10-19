#!/usr/bin/env python3
"""Verify the emitted helper artifacts before staging native binaries/licenses."""
from pathlib import Path
import hashlib
import json
import shutil

root=Path(__file__).resolve().parent.parent
source=root/'build/windows-helpers'
(root/'dist').mkdir(exist_ok=True)
manifest=json.loads((source/'SHA256SUMS.json').read_text())
for name in ['backplane-kipc.exe','backplane-step2glb.exe','licenses/bun/LICENSE.md',
             'licenses/step2glb/license.occt.txt','licenses/step2glb/license.occt-import-js.txt']:
    if name not in manifest: raise SystemExit('Missing helper distribution file: '+name)
for name,digest in manifest.items():
    path=source/name
    if not path.resolve().is_relative_to(source.resolve()):
        raise SystemExit('Invalid helper manifest path: '+name)
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit('Helper artifact checksum mismatch: '+name)
for name in ['backplane-kipc.exe','backplane-step2glb.exe']:
    if (source/name).read_bytes()[:2] != b'MZ':
        raise SystemExit('Expected native Windows PE: '+name)
    target=root/'dist'/name
    shutil.copyfile(source/name,target)
    target.chmod(0o755)
shutil.copytree(source/'licenses',root/'dist/licenses',dirs_exist_ok=True)
shutil.copyfile(source/'BUILD.json',root/'dist/WINDOWS-HELPERS.json')
print('WINDOWS_HELPER_ARTIFACTS_VERIFIED_AND_STAGED')
