"""New desktop shell plus exact unchanged retained Electron/runtime inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

p = argparse.ArgumentParser(description=__doc__)
for n in ('source', 'tools', 'retained', 'out'):
    p.add_argument('--'+n, type=Path, required=True)
p.add_argument('--target', required=True)
a = p.parse_args()
s, tools, old, out = (x.resolve() for x in (a.source, a.tools, a.retained, a.out))
assert not out.exists()
os.umask(0)  # receipt modes must survive copies; child-local only
out.mkdir(parents=True)

def sha(f):
    with Path(f).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

pins = {'windows-x64': 'd728eeece0790c1ddd709fa1f94a802f4c85f1a265a8916fc1554876ff9f7628',
        'linux-x64': '9224c533b599816cbf9f4a8bca579a102990b7e3065cdc4c0e9c48652dcb02a6',
        'linux-arm64': '04805e1e6c9fcfac89efef22ce6c8739de9bf4c2160506586d75c484168f343b',
        'darwin-arm64': '100936246e991aeb0c9211b11e3daf1cb4f05ef5f447c03df2e0fa9ad1754441'}
assert sha(old/'components.tar.gz') == pins[a.target]
with tarfile.open(old/'components.tar.gz') as t:
    t.extractall(out/'retained-components', filter='data')
components = out/'retained-components/components'
vendor = out/'vendor'/a.target
vendor.mkdir(parents=True)
electron = json.loads((components/'ELECTRON.json').read_text())
for name, info in electron['members'].items():
    f = components/'payload'/name
    if info['type'] == 'file' and os.name != 'nt':
        assert f.is_file() and not f.is_symlink() and '..' not in Path(name).parts
        f.chmod(info['mode'])
shutil.copytree(components/'payload', vendor/'payload', symlinks=True)
for n in ('ELECTRON.json', 'UPSTREAM.json'):
    if (components/n).exists(): shutil.copyfile(components/n, vendor/n)
ws = s/'desktop/node_modules/ws'
assert not ws.exists()
ws.parent.mkdir(parents=True, exist_ok=True)
shutil.copytree(components/'shell-payload/node_modules/ws', ws, symlinks=False)
new = out/'prepared-components'
subprocess.run(['node', str(tools/'desktop/prepare-components.cjs'), '--source', str(s),
    '--vendor', str(out/'vendor'), '--out', str(new), '--targets', a.target], check=True)
inputs = out/'new-inputs'
inputs.mkdir()
packed = inputs/'components.tar.gz'
with tarfile.open(packed, 'w:gz') as t:
    t.add(new/a.target, arcname='components')
args = []
if a.target == 'windows-x64':
    expected = '42ef24bf69566f3d2d56aebf4b689276831e6e70d629d53d9cb9fc313a357412'
    assert sha(old/'windows-runtime.tar.gz') == expected
    shutil.copyfile(old/'windows-runtime.tar.gz', inputs/'windows-runtime.tar.gz')
    args += ['--windows-sha256', expected]
record = {'sourceCommit': 'be904cd4cb21ad6febe17e674918c96945113d07',
    'target': a.target, 'retainedComponentPacketSha256': pins[a.target],
    'reuseScope': 'Only unchanged upstream Electron/ws and pinned Cygwin dependency bytes. Old shell/service/acceptance is not reused.',
    'newComponentPacketSha256': sha(packed), 'runtimeAcceptance': 'PENDING'}
(out/'COMPONENT-REUSE.json').write_text(json.dumps(record, indent=2)+'\n')
subprocess.run([sys.executable, str(Path(__file__).with_name('native-package.py')),
    '--source', str(s), '--tools', str(tools), '--target', a.target,
    '--authenticated-inputs', str(inputs), '--components-sha256', sha(packed),
    '--out', str(out/'package'), *args], check=True)
