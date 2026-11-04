"""Assemble public source/merge metadata for Boat; executes no checks or compiler.

Git reads and the static source-import inventory pin the currently staged
resolved merge. Tests and proofs run later, only in the remote capped drivers.
"""
import hashlib
import json
from pathlib import Path
import shlex
import subprocess


def git(*args):
    return subprocess.check_output(['git', *args])


head = git('rev-parse', 'HEAD').decode().strip()
main = git('rev-parse', 'origin/main').decode().strip()
tree = git('write-tree').decode().strip()
folder = Path('extra/restructure/pr-wire-final-snapshot')
folder.mkdir(parents=True, exist_ok=True)
for name in ['LAWS.bend', 'PROOF.bend']:
    (folder/(name+'.in')).write_bytes(git('show', tree+':'+name))
paths = ['LAWS.bend', 'PROOF.bend', 'src/server/server.bend', 'src/server/wire.bend',
         'src/app/link.bend', 'src/app/main.bend', 'test/mailcredit_test.bend',
         'test/native/hub_link_test.bend', 'test/native/link_bench.bend']
(folder/'code.patch').write_bytes(git('diff', main, tree, '--', *paths))
base_files = {}
for name in paths:
    result = subprocess.run(['git', 'show', main+':'+name], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if result.returncode == 0:
        destination = folder/'base'/(name+'.in')
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(result.stdout)
        base_files[name] = destination.as_posix()
names = subprocess.check_output(['python3', 'scripts/bend-closure.py', 'PROOF.bend'], text=True).splitlines()
sources = {name: hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in names}
original = json.loads(Path('extra/restructure/wire-identity.json').read_text())
native_files = set(sources) | {
    'scripts/proof-shards.py', 'scripts/heavy-guard.py', 'scripts/bendok.sh', 'scripts/ci-packet.py',
    'scripts/laws-shape.py', 'scripts/bend-closure.py', '.github/actions/setup-bend/action.yml',
    'extra/restructure/run-feature-proofs.py', '.gitignore'}
native = {'requested_laws': original['requested_laws'], 'canonical_sources': sources,
          'toolchain': original['toolchain'],
          'files': {f: hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in sorted(native_files)}}
native_id = Path('extra/restructure/wire-final-identity.json')
native_id.write_text(json.dumps(native, indent=2)+'\n')
files = native_files | set(paths) | {
    'scripts/dependency-map.py', 'scripts/proof-slice.py', 'test/tools/dependency_map_test.py',
    'test/tools/proof_shards_test.py', 'test/tools/proof_slice_test.py', 'extra/restructure/run-pr-checks.py'}
files |= {p.as_posix() for p in folder.rglob('*') if p.is_file()}
pr = {'files': {name: hashlib.sha256(Path(name).read_bytes()).hexdigest() for name in sorted(files)},
      'base_files': base_files, 'code_patch': (folder/'code.patch').as_posix(),
      'merge_laws': (folder/'LAWS.bend.in').as_posix(), 'merge_proof': (folder/'PROOF.bend.in').as_posix(),
      'merge_snapshot': {'main': main, 'head_before_merge_commit': head, 'resolved_staged_tree': tree,
                         'base': main, 'conflict_resolution': 'Both existing disconnect cleanup hooks retained'}}
pr_id = Path('extra/restructure/pr-wire-final-identity.json')
pr_id.write_text(json.dumps(pr, indent=2)+'\n')
files |= {native_id.as_posix(), pr_id.as_posix()}
argv = ['scripts/boat-job.sh', '--size', 'default', '--idle', '0', '--timeout', '900',
        '--artifact', 'build/restructure-wire-final', '--artifact', 'build/restructure-pr-wire-final']
for name in sorted(files):
    argv += ['--include-file', name]
inner = ('python3 extra/restructure/run-feature-proofs.py --identity extra/restructure/wire-final-identity.json '
         '--out build/restructure-wire-final && python3 extra/restructure/run-pr-checks.py '
         '--identity extra/restructure/pr-wire-final-identity.json --out build/restructure-pr-wire-final')
argv += ['--', 'env', 'BACKPLANE_HEAVY_BOAT_SANDBOX=1', 'BACKPLANE_HEAVY_OFFLOADED=1',
         'PYTHONDONTWRITEBYTECODE=1', 'sh', '-c', inner]
Path('build/restructure/WIRE-FINAL-COMMAND.txt').write_text(shlex.join(argv)+'\n')
print(json.dumps({'snapshot': pr['merge_snapshot'], 'canonical_sources': len(sources),
                  'transport_files': len(files), 'command_file': 'build/restructure/WIRE-FINAL-COMMAND.txt'}))
