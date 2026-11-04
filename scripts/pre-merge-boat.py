#!/usr/bin/env python3
"""Boat-only merged-law shape guard. No local merge-tree or validation fallback."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
REMOTE = 'https://github.com/BackplaneWorks/Backplane.git'
CAP = 536870912
GUARD_FILES = ('scripts/pre-merge.sh', 'scripts/pre-merge-boat.py', 'scripts/boat-job.sh')
HEX40 = re.compile(r'[0-9a-f]{40}')
SCOPE_CMD = ('set -eu; unit=bp-premerge-$$.scope; '
             'exec systemd-run --user --scope --quiet --unit="$unit" '
             '-p MemoryMax=536870912 -p MemorySwapMax=0 -- '
             'python3 scripts/pre-merge-boat.py worker --head "$1" --base "$2" --worker-sha "$3" --guard-change-ack "$4"')
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
    if not group.name.startswith('bp-premerge-'):
        raise ValueError('pre-merge worker does not own an isolated Boat executor scope')
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



def guard_files(repo, base, head):
    def snapshot(commit):
        entries = {}
        data = git(repo, 'ls-tree', '-z', commit, '--', *GUARD_FILES)
        for row in data.split(b'\0'):
            if not row: continue
            metadata, name = row.decode().split('\t', 1)
            mode, kind, _ = metadata.split()
            if kind != 'blob' or mode not in ('100644', '100755'):
                raise ValueError('guard file must be a regular source blob')
            entries[name] = {'mode': mode, 'sha256': sha(git(repo, 'show', commit+':'+name))}
        return entries
    before, after = snapshot(base), snapshot(head)
    matrix = {name: {'base': before.get(name), 'head': after.get(name)} for name in GUARD_FILES}
    matched = all(row['base'] is not None and row['base'] == row['head'] for row in matrix.values())
    return matrix, matched


def guard_authorized(head, matched, acknowledgement):
    if matched:
        return acknowledgement == ''
    return isinstance(acknowledgement, str) and acknowledgement == head and HEX40.fullmatch(head) is not None


def guard_matrix_ok(matrix, matched):
    if not isinstance(matrix, dict) or set(matrix) != set(GUARD_FILES):
        return False
    for row in matrix.values():
        if not isinstance(row, dict) or set(row) != {'base', 'head'}:
            return False
        for item in row.values():
            if item is not None and (not isinstance(item, dict) or set(item) != {'mode', 'sha256'}
                    or item['mode'] not in ('100644', '100755')
                    or not isinstance(item['sha256'], str)
                    or re.fullmatch(r'[0-9a-f]{64}', item['sha256']) is None):
                return False
    actual = all(row['base'] is not None and row['base'] == row['head'] for row in matrix.values())
    return type(matched) is bool and matched == actual


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


def worker(head, base, expected, acknowledgement):
    group = worker_group()
    out = ROOT/'build/pre-merge';out.mkdir(parents=True, exist_ok=True)
    if (out/'RESULT.json').exists():
        raise ValueError('existing pre-merge attempt; inspect it instead of overwriting')
    report = {'schema': 3, 'kind': 'boat-merged-laws-shape', 'head': head,
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
            matrix, matched = guard_files(repo, base_oid, head)
            report.update({'base': base_oid, 'guard_files': matrix, 'guard_files_match_base': matched,
                           'guard_change_ack': acknowledgement})
            if not guard_authorized(head, matched, acknowledgement):
                if matched:
                    raise ValueError('guard sources match base; remove the needless guard-change acknowledgement')
                print('pre-merge guard source matrix: '+json.dumps(matrix, sort_keys=True), flush=True)
                raise ValueError('guard files differ from base; after reviewing those changes use '
                                 '--allow-reviewed-guard-change '+head)
            shape_path, driver_sha = trusted_verifiers(repo, base_oid, temp/'trusted')
            report.update({'verification_base': base_oid, 'merge_driver_sha256': driver_sha})
            # Dedicated guard controls also run only here, under the same cap.
            tests = temp/'controls';tests.mkdir()
            (tests/'pre-merge-boat.py').write_bytes(boot)
            (tests/'laws-shape.py').write_bytes(shape_path.read_bytes())
            (tests/'merge-laws.py').write_bytes((temp/'trusted/merge-laws.py').read_bytes())
            control_source = git(repo, 'show', head+':test/tools/pre_merge_boat_test.py')
            (tests/'pre_merge_boat_test.py').write_bytes(control_source)
            controls = subprocess.run([sys.executable, '-IB', '-m', 'unittest', 'discover', '-s', str(tests),
                                       '-p', 'pre_merge_boat_test.py'], capture_output=True, text=True, timeout=60)
            control_log = controls.stdout+controls.stderr
            (out/'controls.txt').write_text(control_log)
            report.update({'controls_exit': controls.returncode, 'controls_sha256': sha(control_log.encode()),
                           'controls_source_sha256': sha(control_source)})
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
            report['state'] = 'passed' if matched else 'passed-reviewed-guard-change'
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report['error'] = str(error)
    report['resources'] = resources(group)
    if not resource_ok(report['resources']):
        report['state'] = 'failed';report['error'] = 'pre-merge resource/OOM contract failed'
    (out/'RESULT.json').write_text(json.dumps(report, sort_keys=True, indent=2)+'\n')
    print(json.dumps(report, sort_keys=True), flush=True)
    return 0 if report['state'] in ('passed', 'passed-reviewed-guard-change') else 1


def receipt_ok(report, head, expected, acknowledgement=''):
    return (isinstance(report, dict) and type(report.get('schema')) is int and report['schema'] == 3
            and report.get('kind') == 'boat-merged-laws-shape' and report.get('head') == head
            and report.get('verification_base') == report.get('base')
            and report.get('worker_sha256') == expected
            and type(report.get('guard_files_match_base')) is bool
            and guard_matrix_ok(report.get('guard_files'), report['guard_files_match_base'])
            and guard_authorized(head, report['guard_files_match_base'], acknowledgement)
            and report.get('guard_change_ack') == acknowledgement
            and report.get('state') == ('passed' if report['guard_files_match_base'] else 'passed-reviewed-guard-change')
            and type(report.get('controls_exit')) is int and report['controls_exit'] == 0
            and type(report.get('shape_exit')) is int and report['shape_exit'] == 0
            and all(isinstance(report.get(n), str) and HEX40.fullmatch(report[n]) for n in ('base', 'tree'))
            and all(isinstance(report.get(n), str) and re.fullmatch(r'[0-9a-f]{64}', report[n])
                    for n in ('laws_sha256', 'proof_sha256', 'shape_sha256', 'merge_driver_sha256', 'controls_sha256', 'controls_source_sha256'))
            and resource_ok(report.get('resources')))


def runner_source(path=None, expected='', transport='include-file'):
    if transport not in ('include-file', 'exclude'):
        raise ValueError('unsupported runner transport')
    if path is None:
        if expected or transport != 'include-file':
            raise ValueError('runner digest/transport override needs an explicit runner path')
        path = ROOT/'scripts/boat-job.sh'
    else:
        path = Path(path)
        if not path.is_absolute() or re.fullmatch(r'[0-9a-f]{64}', expected) is None:
            raise ValueError('explicit runner needs an absolute path and complete SHA256')
    if path.is_symlink() or not stat.S_ISREG(path.stat().st_mode) or not os.access(path, os.X_OK):
        raise ValueError('runner must be an executable regular source file, not a symlink')
    data = path.read_bytes()
    digest = sha(data)
    if expected and digest != expected:
        raise ValueError('runner source differs from the explicitly reviewed SHA256')
    return {'path': str(path), 'sha256': digest, 'transport': transport}, data


def transport_arguments(mode, names):
    worker = 'scripts/pre-merge-boat.py'
    if mode == 'include-file':
        return ['--include-file', worker]
    if mode != 'exclude' or worker not in names:
        raise ValueError('exclude transport needs a complete inventory containing the worker')
    def literal(value):
        # Escape glob syntax for the old runner's fnmatch exclusions.
        return ''.join('[[]' if c == '[' else '[*]' if c == '*' else '[?]' if c == '?' else c for c in value)
    excluded = set()
    for name in names:
        if any(ord(c) < 32 for c in name) or name.startswith('/') or '..' in name.split('/'):
            raise ValueError('unsupported transport inventory path')
        if name == worker:
            continue
        prefix = 'scripts/' if name.startswith('scripts/') else ''
        tail = name[len(prefix):]
        part = tail.split('/', 1)[0]
        excluded.add(literal(prefix+part)+('/*' if '/' in tail else ''))
    return [argument for pattern in sorted(excluded) for argument in ('--exclude', pattern)]


def run_runner(descriptor, data, arguments):
    # bash reads its script while running. Execute exactly the reviewed bytes
    # from a private snapshot so edits/rebases cannot alter a productive job.
    with tempfile.TemporaryDirectory(prefix='bp-premerge-runner-') as folder:
        snapshot = Path(folder)/'runner.sh'
        snapshot.write_bytes(data);snapshot.chmod(0o500)
        if sha(snapshot.read_bytes()) != descriptor['sha256']:
            raise ValueError('private runner snapshot identity failed')
        process = subprocess.Popen([str(snapshot), *arguments], cwd=ROOT,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        job = location = None
        ambiguous = False
        for line in process.stdout:
            print(line, end='', flush=True)
            match = JOB.match(line.strip())
            if match:
                if job is not None:
                    ambiguous = True
                else:
                    job, location = match.groups()
        code = process.wait()  # let the owned runner finish cleanup even on ambiguity
        if ambiguous:
            raise ValueError('multiple executor identities; refuse ambiguous pre-merge receipt')
        return code, job, location


def dispatch(base, acknowledgement='', runner_path=None, runner_sha='', transport='include-file'):
    # Local side: metadata/transport/byte identities only. Never merge or test.
    base_remote, base_local = base_ref(base)
    head = git(ROOT, 'rev-parse', 'HEAD').decode().strip()
    expected = sha(Path(__file__).read_bytes())
    sources = {name: sha(git(ROOT, 'show', head+':'+name)) for name in GUARD_FILES}
    if any(sha((ROOT/name).read_bytes()) != digest for name, digest in sources.items()):
        raise ValueError('guard/transport source differs from pushed HEAD; commit and push first')
    if acknowledgement and acknowledgement != head:
        raise ValueError('guard-change acknowledgement must name this exact pushed HEAD')
    runner, runner_bytes = runner_source(runner_path, runner_sha, transport)
    names = git(ROOT, 'ls-files', '-co', '--exclude-standard', '-z').decode().split('\0')
    argv = ['--size', 'default', '--timeout', '240', '--wait', '0', '--idle', '0', '--no-reuse',
            '--no-setup-bend', *transport_arguments(transport, [n for n in names if n]), '--artifact', 'build/pre-merge',
            '--', 'bash', '-c', SCOPE_CMD, 'premerge-scope', head, base, expected, acknowledgement]
    print('pre-merge: reviewed runner '+runner['sha256']+'; transport '+transport, flush=True)
    code, job, location = run_runner(runner, runner_bytes, argv)
    if code:
        return code
    if job is None or Path(location).name != job:
        raise ValueError('owned Boat execution did not yield one identifiable job')
    at = Path(location)
    provenance = json.loads((at/'provenance.json').read_text())
    report = json.loads((at/'artifacts/pre-merge/RESULT.json').read_text())
    if (provenance.get('job') != job or provenance.get('size') != 'default' or type(provenance.get('exit')) is not int or provenance['exit'] != 0
            or provenance.get('revision') != head or provenance.get('source') != str(ROOT)
            or not receipt_ok(report, head, expected, acknowledgement)):
        raise ValueError('owned pre-merge execution/receipt identity or verdict failed')
    if git(ROOT, 'rev-parse', 'HEAD').decode().strip() != head or sha(Path(__file__).read_bytes()) != expected:
        raise ValueError('local HEAD/worker changed during Boat validation')
    if any(sha((ROOT/name).read_bytes()) != digest for name, digest in sources.items()):
        raise ValueError('local guard/transport source changed during Boat validation')
    current_runner, _ = runner_source(runner_path, runner['sha256'] if runner_path else '', transport)
    if current_runner != runner:
        raise ValueError('reviewed runner changed during Boat validation')
    # Fetching/reading immutable Git source bytes is a static identity audit,
    # never local merge-tree or shape execution.
    git(ROOT, 'fetch', '--no-tags', REMOTE, report['base'])
    matrix, matched = guard_files(ROOT, report['base'], head)
    if matrix != report.get('guard_files') or matched != report['guard_files_match_base']:
        raise ValueError('guard-source matrix differs from immutable Git bytes')
    if sha((at/'artifacts/pre-merge/controls.txt').read_bytes()) != report['controls_sha256']:
        raise ValueError('owned controls log identity differs')
    if sha(git(ROOT, 'show', head+':test/tools/pre_merge_boat_test.py')) != report['controls_source_sha256']:
        raise ValueError('controls source differs from pushed head')
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
    (at/'premerge-controller.json').write_text(json.dumps({'kind': 'owned-premerge-controller',
        'job': job, 'head': head, 'base': report['base'], 'tree': report['tree'],
        'runner': runner, 'private_snapshot_sha256': runner['sha256'],
        'worker_result_sha256': sha((at/'artifacts/pre-merge/RESULT.json').read_bytes()),
        'state': report['state'], 'limits': 'Caller-reviewed local runner identity; not independent worker attestation'},
        sort_keys=True, indent=2)+'\n')
    if not matched:
        print('pre-merge: EXPLICIT REVIEWED GUARD CHANGE ACKNOWLEDGED FOR '+head, flush=True)
    print('pre-merge: merged LAWS/PROOF shape ok (Boat '+job+', tree '+report['tree']+')')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest='action', required=True)
    local = actions.add_parser('dispatch');local.add_argument('base', nargs='?', default='origin/main')
    local.add_argument('--allow-reviewed-guard-change', default='', metavar='EXACT_HEAD')
    local.add_argument('--runner', metavar='ABSOLUTE_PATH')
    local.add_argument('--runner-sha256', default='', metavar='SHA256')
    local.add_argument('--runner-transport', choices=('include-file', 'exclude'), default='include-file')
    remote = actions.add_parser('worker');remote.add_argument('--head', required=True)
    remote.add_argument('--base', required=True);remote.add_argument('--worker-sha', required=True)
    remote.add_argument('--guard-change-ack', default='')
    args = parser.parse_args()
    try:
        return dispatch(args.base, args.allow_reviewed_guard_change, args.runner, args.runner_sha256, args.runner_transport) if args.action == 'dispatch' else worker(args.head, args.base, args.worker_sha, args.guard_change_ack)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print('pre-merge: '+str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
