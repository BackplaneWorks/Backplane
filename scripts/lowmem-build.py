#!/usr/bin/env python3
"""One command for the low-memory build: hub, web, mobile and proof shards as
separate parallel normal-size Boat jobs, each under a hard per-worker cap with
no swap. Collects every worker receipt and fails loudly unless each job passed
its cap, ran on the expected source and tools, and produced bytes identical to
the reference emit.

  scripts/lowmem-build.py run --reference REF.json --qualification Q.json \\
        [--proof-dir build/proof] [--jobs hub,web,mobile,proof] [--dry-run]
  scripts/lowmem-build.py reference --from-dir DIR --out REF.json   (from the CURRENT compiler's output)
  scripts/lowmem-build.py worker ...                                 (runs inside a job)

Exit: 0 all gates passed; 1 a gate failed (cap, swap, source/tool, bytes, exit);
20 the low-memory path is not qualified or could not start, nothing was built
(the report says why; a large machine is never used unless --allow-large-fallback
is given, and then the report and a banner say so); 2 usage.
It does not publish or install anything.
"""
import argparse
import concurrent.futures
import hashlib
import importlib.util
import json
import os
import re
import shlex
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GIB = 1024 ** 3
COMPILER_CAP = 3699998720    # the compiler's own cgroup
CONTROLLER_CAP = 536870912   # this controller's cgroup: the sum, 4236870912, stays under 4 GiB
CAP_BYTES = COMPILER_CAP
BOAT_JOB = ROOT / 'scripts' / 'boat-job.sh'
UNQUALIFIED = 20

# id -> (command inside the job, output path relative to the repo). {out} is the job's output folder.
FIXED = {
    'hub': (['bend', 'src/server/main.bend', '-o', '{out}/hub.js'], '{out}'),
    'web': (['bend', 'src/web/index.html', '-o', '{out}/web'], '{out}'),
    'mobile': (['bend', 'src/mobile/index.html', '-o', '{out}/mobile-web'], '{out}'),
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def tree_hashes(path):
    """{relative path: sha256} for a file or a folder tree (symlinks refused)."""
    path = Path(path)
    if path.is_symlink():
        raise ValueError('symlink output refused: %s' % path)
    if path.is_file():
        return {path.name: sha(path.read_bytes())}
    result = {}
    for p in sorted(path.rglob('*')):
        if p.is_symlink():
            raise ValueError('symlink output refused: %s' % p)
        if p.is_file() and not p.name.endswith('.receipt.json'):
            result[p.relative_to(path).as_posix()] = sha(p.read_bytes())
    return result


def git(*args, cwd=ROOT):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()


def source_identity(root=ROOT):
    return {'head': git('rev-parse', 'HEAD', cwd=root), 'tree': git('rev-parse', 'HEAD^{tree}', cwd=root),
            'clean': git('status', '--porcelain', '--untracked-files=no', cwd=root) == ''}


def tool_identity():
    out = {}
    for name in ('bend', 'bun'):
        exe = subprocess.run(['sh', '-c', 'command -v ' + name], capture_output=True, text=True).stdout.strip()
        if not exe:
            out[name] = None
            continue
        ver = subprocess.run([exe, '--version'], capture_output=True, text=True).stdout.strip()
        out[name] = {'version': ver, 'sha256': sha(Path(exe).resolve().read_bytes())}
    return out


# ---------------------------------------------------------------- worker (inside a job)

def meminfo():
    return {k: int(v.split()[0]) * 1024 for k, v in
            (l.split(':', 1) for l in Path('/proc/meminfo').read_text().splitlines()) if k in ('MemTotal', 'SwapTotal', 'SwapFree')}


def swap_counters():
    return {k: int(v) for k, v in (l.split() for l in Path('/proc/vmstat').read_text().splitlines()) if k in ('pswpin', 'pswpout')}


def cgroup_dir():
    """The cgroup this process runs in. LOWMEM_CGROUP_DIR exists for tests; a receipt records the override and is refused."""
    override = os.environ.get('LOWMEM_CGROUP_DIR')
    if override:
        return Path(override), True
    group = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    return Path('/sys/fs/cgroup') / group, False


def cgroup_text(cg, name):
    try:
        return (cg / name).read_text().strip()
    except OSError:
        return None


def cgroup_events(cg):
    text = cgroup_text(cg, 'memory.events') or ''
    return {k: int(v) for k, v in (l.split() for l in text.splitlines() if len(l.split()) == 2)}


def read_enforcement(cg, overridden):
    """What the kernel actually applies to this cgroup, read from its files (never from a flag we were given)."""
    def number(name):
        v = cgroup_text(cg, name)
        return int(v) if v is not None and v.isdigit() else v
    events = cgroup_events(cg)
    return {'executor': 'cgroup-v2', 'cgroup': str(cg), 'overridden': overridden,
            'raw': {n: cgroup_text(cg, n) for n in ('memory.max', 'memory.swap.max', 'memory.peak', 'memory.events')},
            'memory_max': number('memory.max'), 'memory_swap_max': number('memory.swap.max'),
            'memory_peak': number('memory.peak'), 'swap_peak': number('memory.swap.peak'),
            'oom': events.get('oom', 0), 'oom_kill': events.get('oom_kill', 0), 'oom_group_kill': events.get('oom_group_kill', 0)}


def enforcement_problems(e, cap):
    """Reasons the kernel-level admission is not what a qualifying job needs (empty = enforced)."""
    if not isinstance(e, dict):
        return ['no cgroup enforcement record']
    why = []
    if e.get('executor') != 'cgroup-v2' or e.get('overridden'):
        why.append('executor is not an unmodified cgroup v2 scope')
    if e.get('memory_max') != cap:
        why.append('kernel memory.max %r is not the cap %s (unknown/unlimited refused)' % (e.get('memory_max'), cap))
    if e.get('memory_swap_max') != 0:
        why.append('kernel memory.swap.max %r is not 0' % (e.get('memory_swap_max'),))
    peak = e.get('memory_peak')
    if type(peak) is not int or peak <= 0:
        why.append('kernel memory.peak missing')
    elif peak > cap:
        why.append('OVER CAP: kernel memory.peak %.2f GiB > %.2f GiB' % (peak / GIB, cap / GIB))
    if type(e.get('swap_peak')) is int and e['swap_peak'] > 0:
        why.append('SWAP used: memory.swap.peak %d' % e['swap_peak'])
    if (e.get('oom') or 0) or (e.get('oom_kill') or 0) or (e.get('oom_group_kill') or 0):
        why.append('kernel OOM event in the job cgroup')
    return why


def systemd_scope(cap, argv):
    return ['systemd-run', '--user', '--scope', '-q', '--collect', '-p', 'MemoryAccounting=yes',
            '-p', 'MemoryMax=%d' % cap, '-p', 'MemorySwapMax=0', *argv]


def shim(args):
    """Inside the compiler's own cgroup: check the kernel limits, run the command, read the limits and peak back."""
    cg, overridden = cgroup_dir()
    before = read_enforcement(cg, overridden)
    code = None
    if not enforcement_problems({**before, 'memory_peak': 1}, args.cap_bytes):
        code = subprocess.run(args.cmd, cwd=ROOT).returncode
    else:
        print('lowmem-shim: kernel cap not in force; the command was NOT run', file=sys.stderr)
    Path(args.report).write_text(json.dumps({'exit': code, 'enforcement': read_enforcement(cg, overridden)}))
    return 0 if code == 0 else 1


def run_capped(cmd, cap, report):
    """The compiler in a kernel scope of its own (MemoryMax=cap, MemorySwapMax=0); (exit, enforcement) read from inside it."""
    argv = systemd_scope(cap, [sys.executable, str(Path(__file__).resolve()), 'shim', '--cap-bytes', str(cap), '--report', str(report), '--', *cmd])
    try:
        subprocess.run(argv, cwd=ROOT)
        data = json.loads(Path(report).read_text())
        return data.get('exit'), data.get('enforcement')
    except (OSError, ValueError) as error:
        print('lowmem-worker: no enforced compiler scope: %s' % error, file=sys.stderr)
        return None, None


def worker(args):
    """Runs inside one job. The controller is itself held to CONTROLLER_CAP and the command to its own cgroup of
    COMPILER_CAP (swap 0 both): the kernel enforces, and the receipt carries both cgroups read back from inside."""
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    cmd = [c.replace('{out}', str(out)) for c in args.cmd]
    if not os.environ.get('LOWMEM_SCOPED') and not args.no_scope:
        scope = systemd_scope(CONTROLLER_CAP, ['--setenv=LOWMEM_SCOPED=1', '--', sys.executable, str(Path(__file__).resolve()),
                                               'worker', *sys.argv[sys.argv.index('worker') + 1:]])
        try:
            os.execvp(scope[0], scope)
        except OSError as error:
            print('lowmem-worker: no systemd scope to enforce the cap: %s' % error, file=sys.stderr)
    swap_total = meminfo().get('SwapTotal', 0)
    before = swap_counters()
    start = time.monotonic()
    code, compiler = run_capped(cmd, args.cap_bytes, out.parent / (args.id + '.enforcement.json'))
    seconds = time.monotonic() - start
    after = swap_counters()
    try:
        cg, overridden = cgroup_dir()
        controller = read_enforcement(cg, overridden)
    except (OSError, IndexError):
        controller = None
    try:
        outputs = tree_hashes(out)
    except ValueError as error:
        outputs = {}
        print('lowmem-worker: %s' % error, file=sys.stderr)
    if args.copy and Path(ROOT / args.copy).is_file():
        (out / 'result.json').write_bytes((ROOT / args.copy).read_bytes())
        outputs = tree_hashes(out)
    receipt = {'schema': 2, 'id': args.id, 'cmd': cmd, 'exit': code, 'seconds': round(seconds, 2),
               'cap_bytes': args.cap_bytes, 'peak_bytes': (compiler or {}).get('memory_peak'),
               'enforcement': compiler, 'controller': controller, 'swap_total_bytes': swap_total,
               'swap_before': before, 'swap_after': after,
               'swap_activity': after != before, 'mem_total_bytes': meminfo().get('MemTotal'),
               'boat_sandbox': os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == '1',
               'source': source_identity(), 'tools': tool_identity(), 'outputs': outputs}
    (out / (args.id + '.receipt.json')).write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    why = receipt_problems(receipt, args.cap_bytes, require_boat=False)
    print('lowmem-worker: %s exit %s peak %s of %.2f GiB, %.0f s%s' % (
        args.id, code, ('%.2f GiB' % (receipt['peak_bytes'] / GIB)) if isinstance(receipt['peak_bytes'], int) else 'unknown',
        args.cap_bytes / GIB, seconds, ' -- ' + '; '.join(why) if why else ''), flush=True)
    return 0 if code == 0 and not why and bool(outputs) else 1


def receipt_problems(r, cap, require_boat=True):
    """Executor/cap/swap/exit problems of one worker receipt (shared with service-release.py's report check)."""
    if not isinstance(r, dict) or r.get('schema') != 2:
        return ['receipt is not a schema 2 worker receipt']
    why = enforcement_problems(r.get('enforcement'), cap)
    if cap == COMPILER_CAP:
        why += ['controller: ' + w for w in enforcement_problems(r.get('controller'), CONTROLLER_CAP)]
        if cap + CONTROLLER_CAP >= 4 * GIB:
            why.append('compiler plus controller caps must stay under 4 GiB')
    if r.get('cap_bytes') != cap:
        why.append('cap %s is not the required %s' % (r.get('cap_bytes'), cap))
    if not 0 < cap < 4 * GIB:
        why.append('cap must be under 4 GiB')
    if r.get('exit') != 0:
        why.append('worker exit %s' % r.get('exit'))
    if r.get('swap_activity'):
        why.append('SWAP used (kernel swap counters moved)')
    if require_boat and (not r.get('boat_sandbox') or not isinstance(r.get('mem_total_bytes'), int) or r['mem_total_bytes'] > 8 * GIB):
        why.append('not a normal-size Boat sandbox')
    return why


PROOF_WRAPPER = ['python3', 'scripts/heavy-guard.py', 'proof-shard', '--']  # lane 3's profile (PR #854); exits 75 when the cgroup is not <=3.7 GiB with swap 0


def proof_jobs(proof_dir, wrapper=PROOF_WRAPPER):
    if not proof_dir:
        return []
    ps = Path(proof_dir) / 'manifest.json'
    if not ps.is_file():
        return []
    manifest = json.loads(ps.read_text())
    identity = sha((json.dumps(manifest, sort_keys=True, indent=2) + '\n').encode())
    rel = Path(proof_dir).resolve().relative_to(ROOT).as_posix()
    count = len(manifest['shards'])
    jobs = []
    for s in manifest['shards']:
        result = '%s/receipts/%s.json' % (rel, s['id'])
        # the partition is deterministic: a job regenerates it, and --expect pins its identity
        script = ('[ -f %(d)s/manifest.json ] || python3 scripts/proof-shards.py generate --out %(d)s --shards %(n)d >/dev/null; '
                  '%(w)s python3 scripts/proof-shards.py run --out %(d)s --shard %(s)s --expect %(i)s --result %(r)s'
                  % {'d': rel, 'n': count, 'w': shlex.join(wrapper), 's': shlex.quote(s['id']), 'i': identity, 'r': result})
        jobs.append({'id': 'proof-' + s['id'], 'kind': 'proof', 'cmd': ['sh', '-c', script], 'copy': result, 'shard': s['id'], 'identity': identity})
    return jobs


def plan(kinds, proof_dir):
    jobs = [{'id': k, 'kind': k, 'cmd': FIXED[k][0]} for k in ('hub', 'web', 'mobile') if k in kinds]
    if 'proof' in kinds:
        jobs += proof_jobs(proof_dir)
    return jobs


def boat_argv(job, run_dir, timeout, python=sys.executable):
    rel = 'build/lowmem/%s/%s' % (run_dir.name, job['id'])
    worker_cmd = ['python3', 'scripts/lowmem-build.py', 'worker', '--id', job['id'], '--out', rel,
                  '--cap-bytes', str(CAP_BYTES), *(['--copy', job['copy']] if job.get('copy') else []), '--', *[c.replace('{out}', rel) for c in job['cmd']]]
    return [str(BOAT_JOB), '--size', 'default', '--timeout', str(timeout), '--artifact', rel, '--', *worker_cmd], rel


def launch(job, run_dir, args):
    """Run one job; returns (exit code, folder where its artifacts landed or None)."""
    if args.runner == 'local':
        out = run_dir / job['id']
        wc = [sys.executable, str(Path(__file__)), 'worker', '--id', job['id'], '--out', str(out),
              '--cap-bytes', str(args.cap_bytes), *(['--copy', job['copy']] if job.get('copy') else []), '--', *[c.replace('{out}', str(out)) for c in job['cmd']]]
        p = subprocess.run(wc, cwd=ROOT)
        return p.returncode, out
    argv, rel = boat_argv(job, run_dir, args.timeout)
    log = run_dir / (job['id'] + '.log')
    with log.open('w') as handle:
        p = subprocess.run(argv, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT, text=True)
    m = re.search(r'logs and artifacts in (\S+)', log.read_text())
    return p.returncode, (Path(m.group(1)) / 'artifacts' / Path(rel).name if m else None)


def judge(job, receipt, reference, expected_source, expected_tools, cap_bytes, proof=None, require_boat=True):
    """Every reason this job fails the gate (empty list = passed)."""
    if receipt is None:
        return ['no worker receipt (job never finished or artifact lost)']
    why = receipt_problems(receipt, cap_bytes, require_boat)
    if receipt.get('id') != job['id']:
        why.append('receipt belongs to %r' % receipt.get('id'))
    if receipt.get('source') != expected_source:
        why.append('source mismatch: %s' % receipt.get('source'))
    elif not expected_source.get('clean'):
        why.append('source tree is not clean')
    if not expected_tools or receipt.get('tools') != expected_tools:
        why.append('tool provenance mismatch with the qualified compiler: %s' % receipt.get('tools'))
    if not receipt.get('tools') or not receipt['tools'].get('bend'):
        why.append('no bend tool provenance')
    if job['kind'] == 'proof':
        # a proof passes only on the planner's own result semantics; no verdict, no pass
        why.extend(proof if proof is not None else ['proof result was not validated'])
        return why
    want = (reference or {}).get('outputs', {}).get(job['id'])
    if want is None:
        why.append('no reference emit for %s' % job['id'])
    elif receipt.get('outputs') != want:
        diff = sorted(set(want) ^ set(receipt.get('outputs') or {}) |
                      {k for k in want if k in (receipt.get('outputs') or {}) and want[k] != receipt['outputs'][k]})
        why.append('NOT BYTE-IDENTICAL to reference: %s' % ', '.join(diff[:8]))
    return why


def planner():
    spec = importlib.util.spec_from_file_location('proof_shards', ROOT / 'scripts' / 'proof-shards.py')
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def validate_proof(jobs, results, proof_dir, ps=None):
    """{job id: [reasons]} for proof jobs. Each shard result must be the worker-hashed file, name its shard and
    the exact partition identity, and the planner's fail-closed join must accept ALL shards of the verified manifest."""
    proofs = [j for j in jobs if j['kind'] == 'proof']
    if not proofs:
        return {}
    ps = ps or planner()
    out = {j['id']: [] for j in proofs}
    paths = []
    for j in proofs:
        rc, receipt, folder = results.get(j['id'], (None, None, None))
        path = Path(folder) / 'result.json' if folder else None
        if not receipt or not path or not path.is_file():
            out[j['id']].append('shard result missing')
            continue
        data = path.read_bytes()
        if (receipt.get('outputs') or {}).get('result.json') != sha(data):
            out[j['id']].append('shard result does not match the worker-hashed bytes')
            continue
        try:
            r = json.loads(data)
        except ValueError:
            out[j['id']].append('shard result is not JSON')
            continue
        if r.get('shard') != j['shard'] or r.get('manifest_sha256') != j['identity'] or r.get('state') != 'passed':
            out[j['id']].append('shard result is not a passing result for this shard and partition (state %r)' % r.get('state'))
            continue
        paths.append(path)
    try:
        manifest = ps.verify(ROOT, Path(proof_dir).resolve())
        joined = ps.join(manifest, Path(proof_dir).resolve(), paths)
        if joined.get('state') != 'passed':
            raise ValueError('join did not pass')
    except (ValueError, OSError, KeyError, TypeError) as error:
        for k in out:
            out[k].append('proof join refused: %s' % error)
    return out


def load_json(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None


def qualified(path, source):
    """(ok, reason, qualification). It must name this exact source tree and the low-memory compiler it qualifies."""
    q = load_json(path) if path else None
    if not isinstance(q, dict):
        return False, 'no readable qualification file', None
    if q.get('qualified') is not True:
        return False, q.get('reason', 'compiler is not marked qualified'), None
    tools = q.get('tools')
    if not isinstance(tools, dict) or not isinstance(tools.get('bend'), dict) or not tools['bend'].get('sha256'):
        return False, 'qualification does not name the compiler it qualifies', None
    if (q.get('source') or {}).get('tree') != source['tree']:
        return False, 'qualification is for another source tree', None
    return True, '', q


def ref_to(path, base):
    path = Path(path)
    return {'path': path.relative_to(base).as_posix(), 'sha256': sha(path.read_bytes()), 'bytes': path.stat().st_size}


def banner(text):
    line = '!' * 72
    print('\n%s\n%s\n%s\n' % (line, text, line), file=sys.stderr, flush=True)


def run(args):
    kinds = set(args.jobs.split(','))
    if not kinds <= {'hub', 'web', 'mobile', 'proof'}:
        print('unknown job kind', file=sys.stderr)
        return 2
    jobs = plan(kinds, args.proof_dir)
    source = source_identity()
    ok, reason, qual = qualified(args.qualification, source)
    stamp = time.strftime('%Y%m%dT%H%M%S')
    run_dir = Path(args.workdir) / ('lowmem-' + stamp)
    if args.dry_run:
        print('DRY RUN: nothing is launched')
        print('qualified: %s%s' % (ok, '' if ok else ' (%s)' % reason))
        print('runner: %s, cap %.2f GiB per job, swap 0, size default (never xlarge)' % (args.runner, args.cap_bytes / GIB))
        for j in jobs:
            print('  %-22s %s' % (j['id'], shlex.join(j['cmd'])))
            if args.runner == 'boat':
                print('  %-22s %s' % ('', shlex.join(boat_argv(j, run_dir, args.timeout)[0])))
        if 'proof' in kinds and not any(j['kind'] == 'proof' for j in jobs):
            print('  proof: no manifest at --proof-dir; run scripts/proof-shards.py generate first')
        return 0
    report = {'schema': 2, 'kind': 'backplane-lowmem-build-report', 'started': stamp, 'runner': args.runner, 'qualified': ok, 'fallback': False, 'jobs': {}}
    if args.release and (args.runner != 'boat' or not {'hub', 'web', 'mobile'} <= kinds):
        print('--release needs the boat runner and all of hub, web and mobile', file=sys.stderr)
        return 2
    if not ok or not jobs:
        reason = reason or 'no jobs planned (proof manifest missing?)'
        report.update(state='unqualified', reason=reason)
        return unqualified(args, report, reason)
    run_dir.mkdir(parents=True, exist_ok=True)
    reference = load_json(args.reference) if args.reference else None
    tools = qual['tools']
    if not isinstance(reference, dict) or (reference.get('source') or {}).get('tree') != source['tree']:
        report.update(state='unqualified', reason='reference emit is missing or for another source tree')
        return unqualified(args, report, report['reason'])
    t0 = time.monotonic()
    results = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.parallel or len(jobs)) as pool:
        futures = {pool.submit(launch, j, run_dir, args): j for j in jobs}
        for f in concurrent.futures.as_completed(futures):
            j = futures[f]
            try:
                rc, folder = f.result()
            except Exception as error:  # a job that cannot even start is a failure, not a pass
                rc, folder = 99, None
                report['jobs'].setdefault(j['id'], {})['launch_error'] = str(error)
            receipt = load_json(folder / (j['id'] + '.receipt.json')) if folder else None
            if receipt is None and rc in (75, 76, 77):
                report['jobs'].setdefault(j['id'], {})['launch_refused'] = rc
            results[j['id']] = (rc, receipt, folder)
            print('job %-22s exit %s' % (j['id'], rc), flush=True)
    wall = time.monotonic() - t0
    proof_check = validate_proof(jobs, results, args.proof_dir)
    failed = {}
    for j in jobs:
        rc, receipt, folder = results[j['id']]
        why = judge(j, receipt, reference, source, tools, args.cap_bytes,
                    proof_check.get(j['id'], ['no proof result']) if j['kind'] == 'proof' else None,
                    require_boat=args.runner == 'boat')
        if rc != 0 and not why:
            why.append('boat-job exit %s' % rc)
        if rc == 75 or (receipt or {}).get('exit') == 75:
            why.append('REFUSED (exit 75): the job was not admitted at the required cap; reported as fallback, never a pass')
        entry = report['jobs'].setdefault(j['id'], {})
        entry.update(exit=rc, seconds=(receipt or {}).get('seconds'), peak_bytes=(receipt or {}).get('peak_bytes'),
                     ok=not why, why=why, kind=j['kind'], outputs=(receipt or {}).get('outputs'))
        if receipt is not None:  # an immutable copy of the worker receipt, bound by hash
            dest = run_dir / 'receipts' / (j['id'] + '.receipt.json')
            dest.parent.mkdir(exist_ok=True)
            dest.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
            entry['receipt'] = ref_to(dest, run_dir)
        if why:
            failed[j['id']] = why
    report.update(source=source, tools=tools, cap_bytes=args.cap_bytes, release=bool(args.release),
                  complete={'hub', 'web', 'mobile'} <= {k for k, e in report['jobs'].items() if e.get('ok')},
                  qualification=ref_to(Path(args.qualification), Path(args.qualification).parent),
                  reference=ref_to(Path(args.reference), Path(args.reference).parent),
                  wall_seconds=round(wall, 1), state='failed' if failed else 'passed',
                  peak_bytes=max([e.get('peak_bytes') or 0 for e in report['jobs'].values()] or [0]))
    write_report(run_dir, report)
    print('\nend-to-end wall %.0f s, %d jobs, peak per job:' % (wall, len(jobs)))
    for k, e in sorted(report['jobs'].items()):
        print('  %-22s %s  peak %s  %s' % (k, 'ok  ' if e.get('ok') else 'FAIL',
              '%.2f GiB' % (e['peak_bytes'] / GIB) if e.get('peak_bytes') else 'n/a', '; '.join(e.get('why', []))))
    if args.release and not report['complete'] and not failed:
        failed['report'] = ['release needs hub, web and mobile all passing']
        report['state'] = 'failed'
        write_report(run_dir, report)
    if failed:
        banner('LOW-MEMORY BUILD FAILED: %s' % ', '.join(sorted(failed)))
        return 1
    print('low-memory build passed: every job under %.2f GiB, no swap, byte-identical to the reference. Report: %s' % (
        args.cap_bytes / GIB, run_dir / 'report.json'))
    return 0


def write_report(run_dir, report):
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / 'report.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')


def unqualified(args, report, reason):
    banner('LOW-MEMORY PATH NOT USED: %s\nNothing was built here. No silent switch to a large machine.' % reason)
    if args.allow_large_fallback and args.fallback_cmd:
        banner('EXPLICIT FALLBACK to the large path (--allow-large-fallback): %s' % args.fallback_cmd)
        report.update(fallback=True, fallback_cmd=args.fallback_cmd)
        write_report(Path(args.workdir) / ('lowmem-' + report['started']), report)
        return subprocess.run(args.fallback_cmd, shell=True, cwd=ROOT).returncode
    write_report(Path(args.workdir) / ('lowmem-' + report['started']), report)
    return UNQUALIFIED


def make_reference(args):
    d = Path(args.from_dir)
    outputs = {}
    for k in ('hub', 'web', 'mobile'):
        p = d / k
        if p.exists():
            outputs[k] = tree_hashes(p)
    ref = {'schema': 1, 'source': source_identity(), 'tools': tool_identity(), 'outputs': outputs}
    Path(args.out).write_text(json.dumps(ref, indent=2, sort_keys=True) + '\n')
    print('reference for %s written to %s' % (', '.join(sorted(outputs)) or 'nothing', args.out))
    return 0 if outputs else 1


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='op', required=True)
    r = sub.add_parser('run')
    r.add_argument('--reference')
    r.add_argument('--qualification')
    r.add_argument('--proof-dir')
    r.add_argument('--jobs', default='hub,web,mobile,proof')
    r.add_argument('--runner', choices=('boat', 'local'), default='boat')
    r.add_argument('--release', action='store_true', help='release/deploy path: boat runner only')
    r.add_argument('--cap-bytes', type=int, default=CAP_BYTES)
    r.add_argument('--timeout', type=int, default=3000)
    r.add_argument('--parallel', type=int, default=0)
    r.add_argument('--workdir', default=str(ROOT / 'build'))
    r.add_argument('--dry-run', action='store_true')
    r.add_argument('--allow-large-fallback', action='store_true')
    r.add_argument('--fallback-cmd')
    f = sub.add_parser('reference')
    f.add_argument('--from-dir', required=True)
    f.add_argument('--out', required=True)
    w = sub.add_parser('worker')
    w.add_argument('--id', required=True)
    w.add_argument('--out', required=True)
    w.add_argument('--cap-bytes', type=int, default=CAP_BYTES)
    w.add_argument('--poll', type=float, default=0.1)
    w.add_argument('--no-scope', action='store_true', help='tests only: the receipt will not qualify')
    w.add_argument('--copy')
    w.add_argument('cmd', nargs=argparse.REMAINDER)
    h = sub.add_parser('shim')
    h.add_argument('--cap-bytes', type=int, required=True)
    h.add_argument('--report', required=True)
    h.add_argument('cmd', nargs=argparse.REMAINDER)
    args = p.parse_args(argv)
    if args.op == 'shim':
        if args.cmd[:1] == ['--']:
            args.cmd = args.cmd[1:]
        return shim(args) if args.cmd else 2
    if args.op == 'worker':
        if args.cmd[:1] == ['--']:
            args.cmd = args.cmd[1:]
        if not args.cmd:
            return 2
        return worker(args)
    if args.op == 'reference':
        return make_reference(args)
    return run(args)


if __name__ == '__main__':
    raise SystemExit(main())
