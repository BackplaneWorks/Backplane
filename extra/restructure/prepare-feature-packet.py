"""Read staged source/Git metadata and assemble a Boat-only feature check packet.

This performs source inventory and transport preparation, never a compiler,
proof checker, unit test, shape check or merge-tree validation locally.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--label', required=True)
parser.add_argument('--law', action='append', required=True)
parser.add_argument('--path', action='append', required=True)
parser.add_argument('--toolchain', default='extra/restructure/wire-final-identity.json')
args = parser.parse_args()
if not re.fullmatch(r'[a-z][a-z0-9-]+', args.label):
    raise SystemExit('label must be a plain lowercase name')


def git(*argv):
    return subprocess.check_output(['git', *argv])


label = args.label
main = git('rev-parse', 'origin/main').decode().strip()
head = git('rev-parse', 'HEAD').decode().strip()
tree = git('write-tree').decode().strip()
snapshot = Path('extra/restructure')/(label+'-snapshot')
snapshot.mkdir(parents=True, exist_ok=True)
for name in ['LAWS.bend', 'PROOF.bend']:
    (snapshot/(name+'.in')).write_bytes(git('show', tree+':'+name))
(snapshot/'code.patch').write_bytes(git('diff', main, tree, '--', *args.path))
base_files = {}
for name in args.path:
    result = subprocess.run(['git', 'show', main+':'+name], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if result.returncode == 0:
        path = snapshot/'base'/(name+'.in')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(result.stdout)
        base_files[name] = path.as_posix()
names = subprocess.check_output(['python3', 'scripts/bend-closure.py', 'PROOF.bend'], text=True).splitlines()
sources = {name: hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in names}
toolchain = json.loads(Path(args.toolchain).read_text())['toolchain']
files = set(sources) | {
    'scripts/proof-shards.py', 'scripts/heavy-guard.py', 'scripts/bendok.sh', 'scripts/ci-packet.py',
    'scripts/laws-shape.py', 'scripts/bend-closure.py', '.github/actions/setup-bend/action.yml',
    'extra/restructure/run-feature-proofs.py', '.gitignore'}
native = {'requested_laws': sorted(set(args.law)), 'canonical_sources': sources, 'toolchain': toolchain,
          'files': {f: hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in sorted(files)}}
native_id = Path('extra/restructure')/(label+'-identity.json')
native_id.write_text(json.dumps(native, indent=2)+'\n')
files |= set(args.path) | {
    'scripts/dependency-map.py', 'scripts/proof-slice.py', 'test/tools/dependency_map_test.py',
    'test/tools/proof_shards_test.py', 'test/tools/proof_slice_test.py', 'extra/restructure/run-pr-checks.py'}
files |= {p.as_posix() for p in snapshot.rglob('*') if p.is_file()}
pr = {'files': {f: hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in sorted(files)},
      'base_files': base_files, 'code_patch': (snapshot/'code.patch').as_posix(),
      'merge_laws': (snapshot/'LAWS.bend.in').as_posix(), 'merge_proof': (snapshot/'PROOF.bend.in').as_posix(),
      'merge_snapshot': {'main': main, 'head_before_source_commit': head, 'resolved_staged_tree': tree}}
pr_id = Path('extra/restructure')/(label+'-pr-identity.json')
pr_id.write_text(json.dumps(pr, indent=2)+'\n')
files |= {native_id.as_posix(), pr_id.as_posix()}
native_out = 'build/restructure-'+label
pr_out = 'build/restructure-pr-'+label
command = ['/home/h/Documents/i2cjak/bp-coord/bin/boat-job-day-20261103.sh',
           '--size', 'default', '--idle', '0', '--timeout', '900',
           '--artifact', native_out, '--artifact', pr_out]
# This normal daily launcher transports tracked and unignored source inputs.
# It predates --include-file; the identity maps still fence the exact inputs.
inner = (shlex.join(['python3', 'extra/restructure/run-feature-proofs.py', '--identity', native_id.as_posix(), '--out', native_out])
         + ' && ' + shlex.join(['python3', 'extra/restructure/run-pr-checks.py', '--identity', pr_id.as_posix(), '--out', pr_out]))
command += ['--', 'env', 'BACKPLANE_HEAVY_BOAT_SANDBOX=1', 'BACKPLANE_HEAVY_OFFLOADED=1',
            'PYTHONDONTWRITEBYTECODE=1', 'sh', '-c', inner]
output = Path('build/restructure')/(label+'-command.txt')
output.write_text(shlex.join(command)+'\n')
print(json.dumps({'snapshot': pr['merge_snapshot'], 'canonical_sources': len(sources),
                  'transport_files': len(files), 'requested_laws': native['requested_laws'], 'command_file': str(output)}))
