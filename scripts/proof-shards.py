#!/usr/bin/env python3
"""Exact canonical proof partitions. Planning is cheap; execution needs a guarded worker.

No successful partition is a full gate until join verifies every worker receipt.
This does not cache a pass or change the canonical laws/proofs.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import math
import resource
import time
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
HEADER = re.compile(r'^(law|def) ([A-Za-z_][A-Za-z_0-9.]*)[(:]')
IMPORT = re.compile(r'^import (Base|(?:\./|\.\./)[\w./-]+\.bend)(?: as ([A-Za-z_]\w*))?(?:\s*#.*)?$')
LEX = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|#[^\n]*|([A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*)')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT/'scripts'/name)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def tokens(text):
    return {m[1] for m in LEX.finditer(text) if m[1]}


def parse(text, origin):
    """Only the reviewed canonical grammar. Preserve every declaration byte."""
    imports, blocks = {}, []
    current = None
    body_seen = False
    for line in text.splitlines(keepends=True):
        if line and not line[0].isspace() and not line.startswith('#'):
            stripped = line.rstrip('\r\n')
            match = IMPORT.fullmatch(stripped)
            if match:
                if body_seen:
                    raise ValueError('import after a declaration: ' + origin)
                alias = match[2] or match[1]
                if alias in imports or (match[1] != 'Base' and match[2] is None):
                    raise ValueError('duplicate/missing import alias: ' + origin)
                imports[alias] = match[1]
                current = None
                continue
            match = HEADER.match(line)
            if not match:
                raise ValueError('unsupported canonical top-level syntax: ' + stripped)
            body_seen = True
            current = {'kind': match[1], 'name': match[2], 'text': line}
            blocks.append(current)
        elif current is not None:
            current['text'] += line
    if 'Base' not in imports:
        raise ValueError('explicit Base required: ' + origin)
    keys = [(b['kind'], b['name']) for b in blocks]
    if len(keys) != len(set(keys)):
        raise ValueError('duplicate declaration: ' + origin)
    return imports, blocks


def regular(root, path):
    path.relative_to(root)
    if any(p.is_symlink() for p in [path, *path.parents] if p == root or root in p.parents):
        raise ValueError('linked source/partition path: ' + str(path))
    if not path.is_file():
        raise ValueError('missing regular input: ' + str(path))


def model(root):
    for name in ['LAWS.bend', 'PROOF.bend']:
        regular(root, root/name)
    sources = {name: (root/name).read_text() for name in ['LAWS.bend', 'PROOF.bend']}
    errors = module('laws-shape.py').check(sources['LAWS.bend'], sources['PROOF.bend'])
    if errors:
        raise ValueError('\n'.join(errors))
    parsed = {n: parse(s, n) for n, s in sources.items()}
    if parsed['PROOF.bend'][0].get('Laws') != './LAWS.bend':
        raise ValueError('canonical Laws alias changed')
    groups = {}
    for origin, (_, blocks) in parsed.items():
        for b in blocks:
            groups.setdefault((origin, b['name']), []).append(b)
    laws = sorted(b['name'] for b in parsed['LAWS.bend'][1] if b['kind'] == 'law')
    deps, needed = {}, {}
    for key, blocks in groups.items():
        origin, name = key
        used = tokens(''.join(b['text'] for b in blocks))
        prefixes = {t.split('.')[0] for t in used}
        local_prefixes = prefixes - set(parsed[origin][0])
        # Conservative method namespaces: mentioning X includes local X.* too.
        deps[key] = {k for k in groups if k[0] == origin and
                     (k[1] in used or k[1].split('.')[0] in local_prefixes)} - {key}
        needed[key] = {(origin, a) for a in parsed[origin][0] if a in prefixes}
        if origin == 'LAWS.bend' and any(b['kind'] == 'law' for b in blocks):
            deps[key].add(('PROOF.bend', 'Laws.'+name))
        for token in used:
            if origin == 'PROOF.bend' and token.startswith('Laws.'):
                target = token[5:]
                if ('LAWS.bend', target) in groups:
                    deps[key].add(('LAWS.bend', target))
                    if ('PROOF.bend', token) in groups:
                        deps[key].add(('PROOF.bend', token))
    return parsed, groups, laws, deps, needed


def plan(root, folder, count):
    if not 1 <= count <= 128:
        raise ValueError('shards must be between1 and128')
    parsed, groups, laws, deps, needed = model(root)
    roots = sorted({target for im, _ in parsed.values() for target in im.values() if target != 'Base'})
    closure = subprocess.check_output([sys.executable, str(ROOT/'scripts/bend-closure.py'), *roots], cwd=root, text=True).splitlines()
    source = {}
    for name in sorted(set(closure) | {'LAWS.bend', 'PROOF.bend'}):
        p = root/name
        regular(root, p)
        source[name] = sha(p.read_bytes())
    graph = {}
    for name in source:
        edges = set()
        if name.endswith('.bend') and name not in {'LAWS.bend', 'PROOF.bend'}:
            for line in (root/name).read_text().splitlines():
                if not re.match(r'^\s*import\b', line):
                    continue
                match = re.fullmatch(r'\s*import\s+(\S+)(?:\s+as\s+[A-Za-z_]\w*)?\s*(?:#.*)?', line)
                if match is None:
                    raise ValueError('unrecognized dependency import: ' + name)
                target = match[1].strip('"')
                if target == 'Base':
                    continue
                if not target.startswith(('./', '../')):
                    raise ValueError('unsupported nonlocal dependency: ' + name)
                dep = (root/name).parent.joinpath(target).resolve(strict=True).relative_to(root).as_posix()
                if dep not in source:
                    raise ValueError('dependency absent from independent source inventory: ' + dep)
                edges.add(dep)
        graph[name] = edges
    def closed(seed):
        result = set(seed)
        pending = list(seed)
        while pending:
            for dep in deps[pending.pop()]:
                if dep not in result:
                    result.add(dep)
                    pending.append(dep)
        return result

    import_cache = {}
    def import_sources(used):
        result = {'LAWS.bend', 'PROOF.bend'}
        for origin, alias in used:
            if alias in {'Base', 'Laws'}:
                continue
            target = parsed[origin][0][alias]
            if target not in import_cache:
                pending = [(root/target).resolve().relative_to(root).as_posix()]
                found = set()
                while pending:
                    n = pending.pop()
                    if n not in found:
                        found.add(n)
                        pending.extend(graph[n])
                import_cache[target] = found
            result.update(import_cache[target])
        return result

    def imports_for(selected):
        used = set().union(*(needed[k] for k in selected))
        return used | {('LAWS.bend', 'Base'), ('PROOF.bend', 'Base'), ('PROOF.bend', 'Laws')}

    sizes = {n:(root/n).stat().st_size for n in source}
    bins = [set() for _ in range(count)]
    imports = [imports_for(set()) for _ in bins]
    inputs = [import_sources(used) for used in imports]
    primary = [[] for _ in bins]
    weights = [0 for _ in bins]
    maximum = math.ceil(len(laws)/count)
    candidates = {}
    for name in laws:
        selected = closed({('LAWS.bend', name), ('PROOF.bend', 'Laws.'+name)})
        used = imports_for(selected)
        candidates[name] = selected, used, import_sources(used)
    def extra_bytes(i, files):
        return sum(sizes[n] for n in files-inputs[i])
    # Cluster shared dependency closures; equal law-count caps prevent one worker
    # absorbing the suite. This is a source-cost model, not measured proof time.
    for name in sorted(laws, key=lambda n: (-sum(sizes[f] for f in candidates[n][2]), n)):
        selected, used, files = candidates[name]
        i = min((i for i in range(count) if len(primary[i]) < maximum),
                key=lambda i: (extra_bytes(i, files)+10*weights[i], len(primary[i]), i))
        bins[i].update(selected)
        imports[i].update(used)
        inputs[i].update(files)
        primary[i].append(name)
        weights[i] += sum(len(b['text']) for b in groups[('PROOF.bend', 'Laws.'+name)])
    # Every local helper/law/definition is checked somewhere, even when unused.
    assigned = set().union(*bins)
    for key in sorted(groups.keys()-assigned):
        selected = closed({key})
        used = imports_for(selected)
        files = import_sources(used)
        i = min(range(count), key=lambda i: (extra_bytes(i, files), len(bins[i]), i))
        bins[i].update(selected)
        imports[i].update(used)
        inputs[i].update(files)
    # Preserve validation of every original import, including unused proof files.
    all_imports = {(n, a) for n, (im, _) in parsed.items() for a in im}
    for key in sorted(all_imports-set().union(*imports)):
        files = import_sources({key})
        i = min(range(count), key=lambda i: (extra_bytes(i, files), len(imports[i]), i))
        imports[i].add(key)
        inputs[i].update(files)
    generated, shards = {}, []
    for i, selected in enumerate(bins):
        name = f'{i:03d}'
        files = {}
        for origin, (im, blocks) in parsed.items():
            lines = []
            for alias, target in im.items():
                if (origin, alias) not in imports[i]:
                    continue
                if target == 'Base':
                    lines.append('import Base\n')
                elif origin == 'PROOF.bend' and alias == 'Laws':
                    lines.append('import ./LAWS.bend as Laws\n')
                else:
                    regular(root, root/target)
                    at = (root/target).resolve(strict=True)
                    at.relative_to(root)
                    if at.is_symlink():
                        raise ValueError('linked import')
                    relative = os.path.relpath(at, folder/name).replace(os.sep, '/')
                    if not relative.startswith('.'):
                        relative = './' + relative
                    lines.append(f'import {relative} as {alias}\n')
            text = ''.join(lines) + '\n' + ''.join(b['text'] for b in blocks if (origin, b['name']) in selected)
            path = name+'/'+origin
            generated[path] = text.encode()
            files[path] = sha(generated[path])
        shards.append({'id': name, 'primary_laws': sorted(primary[i]),
                       'declarations': [list(k) for k in sorted(selected)],
                       'imports': [list(k) for k in sorted(imports[i])], 'files': files})
    # Independently verify whole-partition coverage; never infer it from a count.
    covered = [n for shard in shards for n in shard['primary_laws']]
    if sorted(covered) != laws or len(covered) != len(set(covered)):
        raise ValueError('canonical law partition is incomplete/duplicated')
    if set().union(*bins) != set(groups) or set().union(*imports) != all_imports:
        raise ValueError('declaration/import coverage incomplete')
    for shard in shards:
        shard['source_files'] = sorted(import_sources({tuple(k) for k in shard['imports']}))
    if set().union(*(set(s['source_files']) for s in shards)) != set(source):
        raise ValueError('shard source union differs from independent closure')
    tool = module('ci-packet.py').toolchain()
    manifest = {'schema': 1, 'kind': 'canonical proof partitions; all receipts required',
                'count': count, 'laws': laws, 'sources': source,
                'source_modes': {n:(root/n).stat().st_mode & 0o777 for n in source}, 'shards': shards,
                'toolchain': tool, 'planner_sha256': sha(Path(__file__).read_bytes()),
                'verdict_sha256': sha((ROOT/'scripts/bendok.sh').read_bytes()),
                'guard_sha256': sha((ROOT/'scripts/heavy-guard.py').read_bytes())}
    return manifest, generated


def verify(root, folder):
    manifest = json.loads((folder/'manifest.json').read_text())
    expected, files = plan(root, folder, manifest['count'])
    if manifest != expected:
        raise ValueError('partition/source/tool identity mismatch')
    for name, body in files.items():
        p = folder/name
        regular(root, p)
        if p.read_bytes() != body:
            raise ValueError('changed generated proof: ' + name)
    return manifest


def verdict(exit_code, text):
    result = subprocess.run(['sh', '-c', '. "$1"; bend_verdict "$2" "$3"',
                             'verdict', str(ROOT/'scripts/bendok.sh'), str(exit_code), text],
                            capture_output=True)
    return result.returncode == 0


def worker_verify(root, folder, manifest, identity, shard):
    if sha(encoded(manifest)) != identity or manifest['planner_sha256'] != sha(Path(__file__).read_bytes()):
        raise ValueError('worker planner/partition identity changed')
    if manifest['toolchain'] != module('ci-packet.py').toolchain():
        raise ValueError('worker compiler/library bytes differ')
    if manifest['verdict_sha256'] != sha((ROOT/'scripts/bendok.sh').read_bytes()) or manifest['guard_sha256'] != sha((ROOT/'scripts/heavy-guard.py').read_bytes()):
        raise ValueError('worker verdict/resource verifier changed')
    for name in shard['source_files']:
        p = root/name
        regular(root, p)
        if sha(p.read_bytes()) != manifest['sources'][name] or p.stat().st_mode & 0o777 != manifest['source_modes'][name]:
            raise ValueError('worker source drift: ' + name)
    for name, digest in shard['files'].items():
        p = folder/name
        regular(root, p)
        if sha(p.read_bytes()) != digest:
            raise ValueError('worker generated proof changed: ' + name)


def execute(root, folder, name, identity, output):
    manifest = json.loads((folder/'manifest.json').read_text())
    shard = next((s for s in manifest['shards'] if s['id'] == name), None)
    if shard is None:
        raise ValueError('unknown shard')
    output = output.resolve()
    output.relative_to(folder/'receipts')
    if output.exists() or output.with_suffix('.started.json').exists():
        raise ValueError('existing shard attempt: inspect/reuse it before any new execution')
    guard = module('heavy-guard.py')
    if guard.inherited_lock(guard.lock_path()) is None:
        raise ValueError('worker needs actual inherited bend flock from its admitted executor')
    group = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    cg = Path('/sys/fs/cgroup')/group
    cap = int((cg/'memory.max').read_text())
    if cap > 7*1024**3 or (cg/'memory.swap.max').read_text().strip() != '0':
        raise ValueError('worker needs a finite <=7GiB no-swap scope')
    resource.setrlimit(resource.RLIMIT_STACK, (1024**3, resource.getrlimit(resource.RLIMIT_STACK)[1]))
    environment = {**os.environ, 'BEND_NO_TELEMETRY':'1', 'DO_NOT_TRACK':'1',
                   'BUN_CONFIG_NO_ANALYTICS':'1', 'BUN_JSC_forceRAMSize':'4294967296',
                   'BUN_JSC_maxPerThreadStackUsage':'536870912'}
    worker_verify(root, folder, manifest, identity, shard)
    output.parent.mkdir(parents=True, exist_ok=True)
    marker = {'shard':name, 'manifest_sha256':identity, 'pid':os.getpid(),
              'pid_start':Path('/proc/self/stat').read_text().split(') ')[1].split()[19],
              'cgroup':group, 'memory_max_bytes':cap, 'started_unix':time.time()}
    with output.with_suffix('.started.json').open('x') as handle:
        handle.write(encoded(marker).decode())
    started = time.monotonic()
    result = subprocess.run(['bend', str(folder/name/'PROOF.bend'), '--check-only'],
                            cwd=root, env=environment, text=True, capture_output=True)
    seconds = time.monotonic()-started
    worker_verify(root, folder, manifest, identity, shard)
    text = result.stdout+result.stderr
    receipt = {'shard':name, 'manifest_sha256':identity,
               'state':'passed' if verdict(result.returncode, text) and resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss*1024 < 3*1024**3 else 'failed',
               'exit':result.returncode, 'output':text, 'seconds':seconds,
               'peak_rss_kib':resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
               'memory_max_bytes':cap, 'aggregate_peak_bytes':int((cg/'memory.peak').read_text()),
               'actual_bend_flock_held':True, 'executor_pid':os.getpid(), 'cgroup':group,
               'inputs_unchanged':True}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded(receipt))
    print(json.dumps({k:v for k,v in receipt.items() if k != 'output'}))
    return 0 if receipt['state']=='passed' else 1


def join(manifest, folder, receipts):
    identity = sha(encoded(manifest))
    if len(receipts) != len(manifest['shards']):
        raise ValueError('missing/extra shard receipts')
    found = {}
    for p in receipts:
        r = json.loads(p.read_text())
        name = r.get('shard')
        if name in found or name not in {s['id'] for s in manifest['shards']}:
            raise ValueError('duplicate/unknown shard receipt')
        if (r.get('manifest_sha256') != identity or r.get('state') != 'passed'
                or not r.get('inputs_unchanged') or not r.get('actual_bend_flock_held')
                or not verdict(r.get('exit'), r.get('output', ''))
                or not all(isinstance(r.get(k), (int, float)) and not isinstance(r[k], bool) and math.isfinite(r[k]) and r[k] > 0 for k in ['seconds', 'peak_rss_kib'])
                or r.get('peak_rss_kib', 0)*1024 >= 3*1024**3
                or not isinstance(r.get('memory_max_bytes'), int) or not 0 < r['memory_max_bytes'] <= 7*1024**3):
            raise ValueError('failed/stale shard receipt')
        found[name] = r
    return {'state': 'passed', 'manifest_sha256': identity, 'law_count': len(manifest['laws']),
            'shards': len(found), 'critical_path_seconds': max(r['seconds'] for r in found.values()),
            'peak_job_rss_kib': max(r['peak_rss_kib'] for r in found.values())}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('action', choices=['generate', 'verify', 'run', 'join'])
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--shards', type=int, default=32)
    p.add_argument('--receipt', type=Path, action='append', default=[])
    p.add_argument('--shard')
    p.add_argument('--expect')
    p.add_argument('--result', type=Path)
    args = p.parse_args()
    root, folder = args.root.resolve(), args.out.resolve()
    folder.relative_to(root)
    if args.action == 'generate':
        manifest, files = plan(root, folder, args.shards)
        if folder.exists():
            raise ValueError('refuse to overwrite an existing partition')
        folder.mkdir(parents=True)
        for name, body in files.items():
            (folder/name).parent.mkdir(exist_ok=True)
            (folder/name).write_bytes(body)
        (folder/'manifest.json').write_bytes(encoded(manifest))
        verify(root, folder)
        print(json.dumps({'manifest_sha256': sha(encoded(manifest)), 'laws': len(manifest['laws']), 'shards': len(manifest['shards'])}))
    elif args.action == 'run':
        if not args.shard or not args.expect or not args.result:
            raise ValueError('run requires --shard, --expect, --result')
        return execute(root, folder, args.shard, args.expect, args.result)
    elif args.action == 'verify':
        manifest = verify(root, folder)
        print(json.dumps({'verified': True, 'laws': len(manifest['laws']), 'shards': len(manifest['shards'])}))
    else:
        print(json.dumps(join(verify(root, folder), folder, args.receipt)))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print('proof-shards: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
