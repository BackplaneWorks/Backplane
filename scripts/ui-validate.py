#!/usr/bin/env python3
"""Bounded pre-push validation. No build, service, network or lock acquisition.

--snapshot is an immutable exported checkout directory of --head. The caller
supplies the exact push base/head and all changed paths, NUL separated. Cache
misses are INCOMPLETE (3). Receipt creation is a separate explicit CI operation.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
import types

# Exported snapshots are immutable; loading the coverage module must not
# leave __pycache__ or any other file beside the source.
sys.dont_write_bytecode = True

START = time.monotonic()
ROOT = Path(__file__).resolve().parent.parent
REPOSITORY = Path.cwd()
SPEC = {'version': 1, 'required': ['static', 'simulator'],
        'buildEnvironment': {'BEND_NO_TELEMETRY':'1','BUN_JSC_forceRAMSize':'4294967296','BUN_JSC_maxPerThreadStackUsage':'536870912'},
        'simulator': {'driver': 'test/tools/ahead_validation.mjs', 'seeds': [1, 3735928559, 41821],
                      'steps': 160, 'valid_parser_cases': 104},
        'layout': 'advisory; measured calibration required in CI',
        'trace': 'structural replay only; synthetic content, not exact text'}


class Incomplete(Exception):
    pass


def sha(data):
    return hashlib.sha256(data).hexdigest()


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        while chunk := f.read(1024*1024):
            h.update(chunk)
    return h.hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def run(args, cwd=None):
    p = subprocess.run(args, cwd=cwd, text=True, capture_output=True,
                       timeout=max(.001, DEADLINE-time.monotonic()))
    if p.returncode:
        raise Incomplete('dependency command failed: ' + args[0])
    return p.stdout.strip()


def files_under(root):
    # Deliberate conservative superset of the UI, transport, parser, route,
    # stylesheet and recorder closure; no test-selection guess can prune it.
    paths = {p.relative_to(root).as_posix() for p in (root/'src').rglob('*')
             if p.is_file() and p.suffix in {'.bend', '.js', '.mjs', '.c', '.h', '.css'}}
    paths.update(p.relative_to(root).as_posix() for p in (root/'scripts/ahead-validation').rglob('*')
                 if p.is_file() and 'node_modules' not in p.parts and p.suffix in {'.py', '.mjs', '.json', '.sh'})
    paths.update(['test/ahead_validation_test.bend', 'test/tools/ahead_validation.mjs',
                  'scripts/ui-validate.py', 'scripts/bend-closure.py', 'scripts/js-native.py',
                  'scripts/js-unmarshal.py', 'LAWS.bend', 'PROOF.bend'])
    return sorted(paths)


def folder_identity(folder):
    if not folder.is_dir():
        raise Incomplete('missing compiler resources')
    rows = {}
    for p in sorted(folder.rglob('*')):
        if p.is_symlink():
            raise Incomplete('linked compiler resource')
        if p.is_file():
            rows[p.relative_to(folder).as_posix()] = digest(p)
    return sha(canonical(rows))


def identity(root):
    inputs = {}
    for path in files_under(root):
        p = root/path
        if not p.is_file() or any(x.is_symlink() for x in [p, *p.parents]):
            raise Incomplete('missing or linked input: ' + path)
        inputs[path] = {'sha256':digest(p), 'executable':bool(p.stat().st_mode & 0o111)}
    tools = {}
    for name in ['bend', 'node']:
        binary = shutil.which(name)
        if not binary:
            raise Incomplete('missing ' + name)
        binary = Path(binary).resolve()
        tools[name] = {'path': str(binary), 'sha256': digest(binary)}
        if name == 'bend':
            tools['bend']['resources'] = folder_identity(binary.parent.parent/'bend2')
    tools['python'] = {'path': sys.executable, 'sha256': digest(sys.executable), 'version': sys.version}
    tools['platform'] = sys.platform
    named = Path(os.environ.get('BEND_LIB', Path.home()/'.bend/lib'))
    tools['named_library'] = folder_identity(named) if named.is_dir() else None
    tools['environment'] = {k: v for k, v in os.environ.items()
                            if k.startswith(('BEND_', 'BUN_', 'NODE_')) or k in ['CC', 'BENDTT', 'LANG', 'LC_ALL']}
    # Actual binary bytes and resources, never a version string alone.
    binding = {'inputs': inputs, 'tools': tools, 'spec': SPEC}
    return sha(canonical(binding)), binding


def coverage(root):
    path = ROOT/'scripts/ahead-validation/coverage.py'
    if digest(path) != digest(root/'scripts/ahead-validation/coverage.py'):
        raise Incomplete('running coverage tool differs from snapshot')
    # Execute the exact hashed source, never an mtime-valid stale .pyc.
    m=types.ModuleType('ui_coverage');m.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),m.__dict__)
    out = m.inventory(m.Source(root=root))
    failed=bool(out['missing'] or not out['remote_image_route_present'] or not out['relative_image_url'] or not out['copy_local_gesture'])
    return {'status': 'fail' if failed else 'incomplete' if out['computed_actions'] or out['unresolved_urls'] else 'pass',
            'literal_contracts':'fail' if failed else 'pass',
            'scope': 'literal action reachability and /img route presence only',
            'bindings': len(out['actions']), 'missing': out['missing'],
            'computed_unresolved': len(out['computed_actions']), 'seconds': out['seconds'],
            'copy_local_gesture':out['copy_local_gesture'],
            'unresolved_urls':len(out['unresolved_urls']),
            'limits': out['limits']}


def exact_snapshot(a, root, paths):
    for value in [a.base, a.head]:
        if not re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', value):
            raise Incomplete('base/head must be full object ids')
    raw = Path(a.changed_paths0).read_bytes()
    if raw and not raw.endswith(b'\0'):
        raise Incomplete('changed-paths0 is not NUL terminated')
    provided = {os.fsdecode(p) for p in raw.split(b'\0') if p}
    # --no-renames emits delete+add, precisely both rename endpoints.
    p = subprocess.run(['git', 'diff', '--no-renames', '--name-only', '-z', a.base, a.head],
                       cwd=REPOSITORY, capture_output=True, timeout=max(.001, DEADLINE-time.monotonic()))
    if p.returncode or provided != {os.fsdecode(x) for x in p.stdout.split(b'\0') if x}:
        raise Incomplete('changed path set differs from pushed commits')
    raw_tree = subprocess.run(['git', 'ls-tree', '-r', '-z', a.head], cwd=REPOSITORY, capture_output=True,
                              timeout=max(.001, DEADLINE-time.monotonic()))
    if raw_tree.returncode:
        raise Incomplete('head tree unavailable')
    tree = {}
    for row in raw_tree.stdout.split(b'\0'):
        if row:
            meta, path = row.split(b'\t', 1)
            mode, kind, oid = meta.decode().split()
            tree[os.fsdecode(path)] = (mode, kind, oid)
    for path in paths:
        record = tree.get(path)
        if not record or record[1] != 'blob' or record[0] not in ['100644', '100755']:
            raise Incomplete('required input is absent from head: '+path)
        body = (root/path).read_bytes()
        algorithm = hashlib.sha256 if len(record[2]) == 64 else hashlib.sha1
        actual = algorithm(b'blob '+str(len(body)).encode()+b'\0'+body).hexdigest()
        if actual != record[2]:
            raise Incomplete('snapshot input differs from head: '+path)
        if bool((root/path).stat().st_mode & 0o111) != (record[0]=='100755'):
            raise Incomplete('snapshot mode differs from head: '+path)
    return len(provided)


def reuse(cache, key):
    try:
        payload=(cache/(key+'.json')).read_bytes()
        if len(payload)>1048576:
            raise ValueError()
        receipt = json.loads(payload)
        checksum = receipt.pop('receipt_sha256')
        if sha(canonical(receipt)) != checksum or sha(canonical(receipt['binding'])) != key:
            raise ValueError()
        if receipt['key'] != key or receipt['spec'] != SPEC:
            raise ValueError()
        result = receipt['result']
        if sha(canonical(result)) != receipt['result_sha256']:
            raise ValueError()
        if result.get('approach') != 'simulator' or result.get('pass') is not True:
            raise ValueError()
        if [(x['seed'], x['steps']) for x in result['seeds']] != [(s,161) for s in SPEC['simulator']['seeds']]:
            raise ValueError()
        if result['parserCases'] != 104 or result['properties']['negativeControls'] < 3:
            raise ValueError()
        module = cache/(key+'.mjs')
        if digest(module) != receipt['module_sha256']:
            raise ValueError()
        if receipt['transport'].get('pass') is not True or receipt['trace'].get('pass') is not True:
            raise ValueError()
        if sha(canonical([receipt['transport'],receipt['trace']])) != receipt['lanes_sha256']:
            raise ValueError()
        return {'status': 'pass', 'cached': True, 'key': key, 'result': result,
                'transport':receipt['transport'],'trace':receipt['trace']}
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, UnicodeError, RecursionError):
        raise Incomplete('simulator cache absent, corrupt or for different inputs')


def main():
    global DEADLINE
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--snapshot', required=True)
    p.add_argument('--base', required=True)
    p.add_argument('--head', required=True)
    p.add_argument('--changed-paths0', required=True)
    p.add_argument('--report', required=True)
    p.add_argument('--budget-ms', type=int, default=3000)
    a = p.parse_args()
    report = {'version':1, 'base':a.base, 'head':a.head, 'approaches': {
        'static': {'status':'incomplete'}, 'simulator': {'status':'incomplete'},
        'layout': {'status':'not-run', 'required':False, 'reason':'CI calibration, advisory'},
        'trace': {'status':'not-run', 'required':False, 'reason':'CI structural replay fixtures'}}}
    rc = 3
    DEADLINE = START + max(0, a.budget_ms)/1000
    def timeout(*_):
        raise Incomplete('budget exhausted')
    signal.signal(signal.SIGALRM, timeout)
    signal.setitimer(signal.ITIMER_REAL, max(.001, DEADLINE-time.monotonic()))
    try:
        root = Path(a.snapshot).resolve(strict=True)
        if not root.is_dir():
            raise Incomplete('snapshot must be an exported checkout directory')
        if Path(a.report).resolve().is_relative_to(root):
            raise Incomplete('report must be outside the immutable snapshot')
        # Always attempt coverage first, even on a cold cache or missing compiler.
        report['approaches']['static'] = coverage(root)
        if digest(__file__) != digest(root/'scripts/ui-validate.py'):
            raise Incomplete('running ui-validate tool differs from snapshot')
        key, binding = identity(root)
        report['changedPaths'] = exact_snapshot(a, root, binding['inputs'])
        cache = Path(os.environ.get('UI_VALIDATE_CACHE', Path.home()/'.cache/backplane-bend/ahead-validation'))
        if cache.resolve().is_relative_to(root):
            raise Incomplete('cache must be outside the immutable snapshot')
        report['approaches']['simulator'] = reuse(cache, key)
        report['approaches']['trace'] = {'status':'incomplete','fixture_status':'pass','cached':True,
            'reason':'structural fixture passed; live capture/export/intake is not installed',
            'result':report['approaches']['simulator'].pop('trace')}
        rc = {'fail':1,'incomplete':3,'pass':0}[report['approaches']['static']['status']]
        if rc==3:
            report['reason']='computed action/URL coverage remains unresolved; scoped literal contracts and simulator passed'
    except (Incomplete, OSError, subprocess.TimeoutExpired) as e:
        report['reason'] = str(e)
        rc = 1 if report['approaches']['static']['status']=='fail' else 3
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    report['status'] = {0:'pass',1:'fail',3:'incomplete'}[rc]
    report['seconds'] = time.monotonic()-START
    out = Path(a.report)
    if out.resolve().is_relative_to(Path(a.snapshot).resolve()):
        print('ui-validate: incomplete (report path would mutate snapshot)')
        return 3
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2)+'\n')
    print(f"ui-validate: {report['status']} ({report['seconds']:.3f}s)")
    return rc


if __name__=='__main__':
    raise SystemExit(main())
