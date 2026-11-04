#!/usr/bin/env python3
"""Boat-only merged-law shape guard. No local merge-tree or validation fallback."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
REMOTE = 'https://github.com/BackplaneWorks/Backplane.git'
CAP = 536870912
HEX40 = re.compile(r'[0-9a-f]{40}')
JOB = re.compile(r'^job (j[0-9]{14}-[0-9]+): logs and artifacts in (.+)$')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(repo, *args):
    result = subprocess.run(['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=0', '-c', 'pack.threads=1', '-c', 'pack.windowMemory=32m', '-C', str(repo), *args], capture_output=True, timeout=120)
    if result.returncode:
        raise ValueError('git operation failed: '+(result.stderr+result.stdout).decode(errors='replace')[:2000])
    return result.stdout


def base_ref(value):
    if HEX40.fullmatch(value):
        return value, None
    if value.startswith('origin/'):
        branch = value[len('origin/'):]
        if (re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9/_-]{0,150}', branch)
                and not branch.endswith('/') and '//' not in branch):
            return 'refs/heads/'+branch, 'refs/remotes/origin/'+branch
    raise ValueError('base must be origin/BRANCH or a complete pushed commit SHA')


def worker_group():
    # Check Boat ownership BEFORE any test, merge-tree, or shape operation.
    if (os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') != '1'
            or os.environ.get('BACKPLANE_HEAVY_OFFLOADED') != '1'
            or os.environ.get('BEND_NO_TELEMETRY') != '1'):
        raise ValueError('pre-merge worker is Boat-only; no local validation permitted')
    relative = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    group = Path('/sys/fs/cgroup')/relative
    if not group.name.startswith('ascii-cmd-'):
        raise ValueError('pre-merge worker does not own an isolated Boat executor scope')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime',
                    group.name, 'MemoryMax='+str(CAP), 'MemorySwapMax=0'], check=True, capture_output=True)
    if (group/'memory.max').read_text().strip() != str(CAP) or (group/'memory.swap.max').read_text().strip() != '0':
        raise ValueError('actual pre-merge cgroup cap/swap contract not enforced')
    return group


def resources(group):
    return {'memory_max_bytes': int((group/'memory.max').read_text()),
            'swap_max_bytes': int((group/'memory.swap.max').read_text()),
            'peak_bytes': int((group/'memory.peak').read_text()),
            'events': {k: int(v) for k, v in (line.split() for line in (group/'memory.events').read_text().splitlines())}}


def resource_ok(value):
    return (isinstance(value, dict) and type(value.get('memory_max_bytes')) is int
            and value['memory_max_bytes'] == CAP and type(value.get('swap_max_bytes')) is int
            and value['swap_max_bytes'] == 0 and type(value.get('peak_bytes')) is int
            and 0 < value['peak_bytes'] <= CAP and isinstance(value.get('events'), dict)
            and all(type(value['events'].get(k)) is int and value['events'][k] == 0
                    for k in ('oom', 'oom_kill', 'oom_group_kill')))


def fetch_base(repo, base):
    remote, local = base_ref(base)
    target = remote+':'+local if local else remote
    git(repo, '-c', 'protocol.version=2', 'fetch', '--filter=blob:none', '--no-tags', 'origin', target)
    return git(repo, 'rev-parse', local or 'FETCH_HEAD').decode().strip()


def inspect_merge(repo, base, head):
    # Exactly Git's actual merge, including conflicts outside LAWS/PROOF.
    output = git(repo, 'merge-tree', '--write-tree', base, head).decode()
    tree = output.splitlines()[0]
    if HEX40.fullmatch(tree) is None:
        raise ValueError('merge-tree did not produce one complete merged tree')
    return tree, git(repo, 'show', tree+':LAWS.bend'), git(repo, 'show', tree+':PROOF.bend')



def trusted_verifiers(repo, base, directory):
    # Candidate changes must never select the checker/driver that judges them.
    directory.mkdir(parents=True, exist_ok=True)
    shape = directory/'laws-shape.py'
    shape.write_bytes(git(repo, 'show', base+':scripts/laws-shape.py'))
    driver_path = directory/'merge-laws.py'
    driver_path.write_bytes(git(repo, 'show', base+':scripts/merge-laws.py'))
    driver = shlex.join([sys.executable, '-IB', str(driver_path)])+' %O %A %B'
    git(repo, 'config', 'merge.laws.driver', driver)
    info = Path(git(repo, 'rev-parse', '--git-path', 'info/attributes').decode().strip())
    if not info.is_absolute(): info = repo/info
    info.parent.mkdir(parents=True, exist_ok=True)
    info.write_text('LAWS.bend merge=laws\nPROOF.bend merge=laws\n')
    return shape, sha(driver_path.read_bytes())


def worker(head, base, expected):
    group = worker_group()
    out = ROOT/'build/pre-merge';out.mkdir(parents=True, exist_ok=True)
    if (out/'RESULT.json').exists():
        raise ValueError('existing pre-merge attempt; inspect it instead of overwriting')
    report = {'schema': 2, 'kind': 'boat-merged-laws-shape', 'head': head,
              'worker_sha256': sha(Path(__file__).read_bytes()), 'state': 'failed'}
    try:
        if HEX40.fullmatch(head) is None or report['worker_sha256'] != expected:
            raise ValueError('worker/head identity mismatch')
        with tempfile.TemporaryDirectory(prefix='bp-premerge-') as temp:
            temp = Path(temp);repo = temp/'repo.git'
            git(temp, 'init', '--bare', str(repo))
            git(repo, 'remote', 'add', 'origin', REMOTE)
            git(repo, 'config', 'remote.origin.promisor', 'true')
            git(repo, 'config', 'remote.origin.partialclonefilter', 'blob:none')
            base_oid = fetch_base(repo, base)
            git(repo, '-c', 'protocol.version=2', 'fetch', '--filter=blob:none', '--no-tags', 'origin', head)
            if git(repo, 'rev-parse', head+'^{commit}').decode().strip() != head:
                raise ValueError('pushed head differs from requested head')
            boot = git(repo, 'show', head+':scripts/pre-merge-boat.py')
            if sha(boot) != expected:
                raise ValueError('worker differs from the pushed reviewed source; push HEAD first')
            shape_path, driver_sha = trusted_verifiers(repo, base_oid, temp/'trusted')
            report.update({'verification_base': base_oid, 'merge_driver_sha256': driver_sha})
            # Dedicated guard controls also run only here, under the same cap.
            tests = temp/'controls';tests.mkdir()
            (tests/'pre-merge-boat.py').write_bytes(boot)
            (tests/'laws-shape.py').write_bytes(shape_path.read_bytes())
            (tests/'merge-laws.py').write_bytes((temp/'trusted/merge-laws.py').read_bytes())
            (tests/'pre_merge_boat_test.py').write_bytes(git(repo, 'show', head+':test/tools/pre_merge_boat_test.py'))
            controls = subprocess.run([sys.executable, '-IB', '-m', 'unittest', 'discover', '-s', str(tests),
                                       '-p', 'pre_merge_boat_test.py'], capture_output=True, text=True, timeout=60)
            (out/'controls.txt').write_text(controls.stdout+controls.stderr)
            if controls.returncode:
                raise ValueError('pre-merge guard controls failed')
            tree, laws, proof = inspect_merge(repo, base_oid, head)
            shape = shape_path.read_bytes()
            (temp/'LAWS.bend').write_bytes(laws);(temp/'PROOF.bend').write_bytes(proof)
            result = subprocess.run([sys.executable, '-IB', str(shape_path),
                                     str(temp/'LAWS.bend'), str(temp/'PROOF.bend')],
                                    capture_output=True, text=True, timeout=60)
            (out/'shape.txt').write_text(result.stdout+result.stderr)
            report.update({'base': base_oid, 'tree': tree, 'shape_exit': result.returncode,
                           'laws_sha256': sha(laws), 'proof_sha256': sha(proof), 'shape_sha256': sha(shape)})
            if result.returncode:
                raise ValueError('actual merged LAWS/PROOF shape failed')
            if fetch_base(repo, base) != base_oid:
                raise ValueError('base advanced during Boat validation; no stale merge permission')
            report['state'] = 'passed'
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = str(error)
    report['resources'] = resources(group)
    if not resource_ok(report['resources']):
        report['state'] = 'failed';report['error'] = 'pre-merge resource/OOM contract failed'
    (out/'RESULT.json').write_text(json.dumps(report, sort_keys=True, indent=2)+'\n')
    print(json.dumps(report, sort_keys=True), flush=True)
    return 0 if report['state'] == 'passed' else 1


def receipt_ok(report, head, expected):
    return (isinstance(report, dict) and type(report.get('schema')) is int and report['schema'] == 2
            and report.get('kind') == 'boat-merged-laws-shape' and report.get('head') == head
            and report.get('verification_base') == report.get('base')
            and report.get('worker_sha256') == expected and report.get('state') == 'passed'
            and type(report.get('shape_exit')) is int and report['shape_exit'] == 0
            and all(isinstance(report.get(n), str) and HEX40.fullmatch(report[n]) for n in ('base', 'tree'))
            and all(isinstance(report.get(n), str) and re.fullmatch(r'[0-9a-f]{64}', report[n])
                    for n in ('laws_sha256', 'proof_sha256', 'shape_sha256', 'merge_driver_sha256'))
            and resource_ok(report.get('resources')))


def dispatch(base):
    # Local side: metadata/transport/byte identities only. Never merge or test.
    base_remote, base_local = base_ref(base)
    head = git(ROOT, 'rev-parse', 'HEAD').decode().strip()
    expected = sha(Path(__file__).read_bytes())
    argv = ['scripts/boat-job.sh', '--size', 'default', '--timeout', '240', '--wait', '0', '--idle', '0', '--no-reuse',
            '--no-setup-bend', '--include-file', 'scripts/pre-merge-boat.py', '--artifact', 'build/pre-merge',
            '--', 'python3', 'scripts/pre-merge-boat.py', 'worker', '--head', head, '--base', base, '--worker-sha', expected]
    process = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    job = location = None
    for line in process.stdout:
        print(line, end='', flush=True)
        match = JOB.match(line.strip())
        if match:
            if job is not None:
                raise ValueError('multiple executor identities; refuse ambiguous pre-merge receipt')
            job, location = match.groups()
    code = process.wait()
    if code:
        return code
    if job is None or Path(location).name != job:
        raise ValueError('owned Boat execution did not yield one identifiable job')
    at = Path(location)
    provenance = json.loads((at/'provenance.json').read_text())
    report = json.loads((at/'artifacts/pre-merge/RESULT.json').read_text())
    if (provenance.get('job') != job or provenance.get('size') != 'default' or type(provenance.get('exit')) is not int or provenance['exit'] != 0
            or provenance.get('revision') != head or provenance.get('source') != str(ROOT)
            or not receipt_ok(report, head, expected)):
        raise ValueError('owned pre-merge execution/receipt identity or verdict failed')
    if git(ROOT, 'rev-parse', 'HEAD').decode().strip() != head or sha(Path(__file__).read_bytes()) != expected:
        raise ValueError('local HEAD/worker changed during Boat validation')
    # Fetching/reading immutable Git source bytes is a static identity audit,
    # never local merge-tree or shape execution.
    git(ROOT, 'fetch', '--no-tags', REMOTE, report['base'])
    if sha(git(ROOT, 'show', report['base']+':scripts/laws-shape.py')) != report['shape_sha256']:
        raise ValueError('shape verifier identity differs from the trusted base')
    if sha(git(ROOT, 'show', report['base']+':scripts/merge-laws.py')) != report['merge_driver_sha256']:
        raise ValueError('merge driver identity differs from the trusted base')
    if base_local:
        current = git(ROOT, 'ls-remote', REMOTE, base_remote).decode().split()
        if not current or current[0] != report['base']:
            raise ValueError('base advanced after Boat validation; do not merge with this receipt')
    elif report['base'] != base_remote:
        raise ValueError('explicit base commit differs from the verified immutable base')
    print('pre-merge: merged LAWS/PROOF shape ok (Boat '+job+', tree '+report['tree']+')')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest='action', required=True)
    local = actions.add_parser('dispatch');local.add_argument('base', nargs='?', default='origin/main')
    remote = actions.add_parser('worker');remote.add_argument('--head', required=True)
    remote.add_argument('--base', required=True);remote.add_argument('--worker-sha', required=True)
    args = parser.parse_args()
    try:
        return dispatch(args.base) if args.action == 'dispatch' else worker(args.head, args.base, args.worker_sha)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print('pre-merge: '+str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
