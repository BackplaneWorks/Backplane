#!/usr/bin/env python3
"""One command for the low-memory build: hub, web, mobile and proof shards as
separate parallel normal-size Boat jobs, each under a hard per-worker cap with
no swap. Collects every worker receipt and fails loudly unless each job passed
its cap, ran on the expected source and tools, and produced bytes identical to
the reference emit.

  scripts/lowmem-build.py run --reference REF.json --qualification Q.json \\
        [--proof-dir build/proof] [--jobs hub,web,mobile,proof] [--dry-run]
  scripts/lowmem-build.py reference --from-emission DIR --out REF.json [--expect-server-sha H]
                                  (only from a saved, verified official producer emission)
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


ENTRY_ROOTS = ['src/server/main.bend', 'src/web/app.bend', 'src/mobile/app.bend']
ENTRY_EXTRA = ['src/web/index.html', 'src/mobile/index.html']
PINNED_ENV = {'BEND_NO_TELEMETRY': '1', 'BUN_JSC_forceRAMSize': '4294967296', 'DO_NOT_TRACK': '1', 'BUN_CONFIG_NO_ANALYTICS': '1'}
PHASES = ['source-closure', 'tools', 'protected', 'flock', 'enforce', 'command', 'outputs']
PROTECTED = ('bend.ts', 'safe.ts', 'base')   # a qualification must name all three
MAX_TIMEOUT = 7200


def closure_identity(root=ROOT):
    """Content AND mode of every file the three entry points read (bend-closure.py, fail closed) plus the HTML entries."""
    files = subprocess.check_output([sys.executable, str(root / 'scripts/bend-closure.py'), *ENTRY_ROOTS], cwd=root, text=True).splitlines()
    files = sorted(set(files) | set(ENTRY_EXTRA))
    modes = {}
    for line in git('ls-files', '-s', '--', *files, cwd=root).splitlines():
        mode, _, _, name = line.split(None, 3)
        modes[name] = mode
    items = []
    for f in files:
        p = root / f
        if f not in modes or p.is_symlink() or not p.is_file():
            raise ValueError('closure member not a tracked regular file: ' + f)
        items.append([f, modes[f], sha(p.read_bytes())])
    return sha(json.dumps(items).encode())


def source_identity(root=ROOT):
    return {'head': git('rev-parse', 'HEAD', cwd=root), 'tree': git('rev-parse', 'HEAD^{tree}', cwd=root),
            'clean': git('status', '--porcelain', '--untracked-files=no', cwd=root) == '',
            'closure_sha256': closure_identity(root)}


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


def controller_cgroup():
    """The cgroup of the nearest ancestor that sits in a lowmem-ctl-*.scope (the launcher's own capped scope).
    Found by walking real process ancestry, so it cannot be named by a flag or an environment variable."""
    pid = os.getppid()
    while pid > 1:
        try:
            group = Path('/proc/%d/cgroup' % pid).read_text().split('0::', 1)[1].strip()
            if re.search(r'/lowmem-ctl-[^/]+\.scope$', group):
                return Path('/sys/fs/cgroup') / group.lstrip('/')
            pid = int(Path('/proc/%d/stat' % pid).read_text().rsplit(')', 1)[1].split()[1])
        except (OSError, IndexError, ValueError):
            return None
    return None


def run_inner(cmd, timeout):
    """The compiler/proof command as a child of the worker, in the SAME kernel cgroup the guard admitted. The bend
    flock descriptor is passed on explicitly (subprocess would close it). (exit, timed_out)."""
    keep = [int(os.environ['BACKPLANE_HEAVY_LOCK_FD'])] if os.environ.get('BACKPLANE_HEAVY_LOCK_FD', '').isdigit() else []
    proc = subprocess.Popen(cmd, cwd=ROOT, env={**os.environ, **PINNED_ENV}, start_new_session=True, pass_fds=keep)
    try:
        return proc.wait(timeout=timeout), False
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)  # a timeout never qualifies
        proc.wait()
        return None, True


REAL_LOCK = '/tmp/bp-bend.lock'


def native_flock():
    """(held, lock path): True only when this process inherited the actual bend flock from heavy-guard. The path
    can be overridden for tests (LOWMEM_LOCK_PATH); a receipt records it and one that is not the real lock never qualifies."""
    spec = importlib.util.spec_from_file_location('heavy_guard', ROOT / 'scripts' / 'heavy-guard.py')
    guard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guard)
    path = Path(os.environ.get('LOWMEM_LOCK_PATH') or guard.lock_path())
    return guard.inherited_lock(path) is not None, str(path)


def worker(args):
    """Runs under heavy-guard, which admitted it inside the COMPILER scope (MemoryMax=cap, MemorySwapMax=0, the bend
    flock inherited). It reads that cgroup's limits from inside, runs the command in the same cgroup, reads them back,
    and also records the controller scope that launched the chain. The kernel enforces; this only reports."""
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    cmd = [c.replace('{out}', str(out)) for c in args.cmd]
    receipt = {'schema': 3, 'id': args.id, 'cmd_template': args.cmd, 'cmd': cmd, 'cap_bytes': args.cap_bytes,
               'env': PINNED_ENV, 'timeout_seconds': args.timeout, 'timed_out': False, 'exit': None,
               'enforcement': None, 'controller': None, 'phases': [], 'flock': False, 'protected': {},
               'outputs': {}, 'output_bytes': {}}
    phases = receipt['phases']
    swap_before = swap_counters()
    start = time.monotonic()
    try:
        receipt['source'] = source_identity(); phases.append('source-closure')
        receipt['tools'] = tool_identity(); phases.append('tools')
        for item in args.protect:
            name, _, path = item.partition('=')
            receipt['protected'][name] = sha(Path(path).read_bytes())
        phases.append('protected')
        receipt['flock'], receipt['flock_path'] = native_flock()
        if receipt['flock']:
            phases.append('flock')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print('lowmem-worker: provenance step failed: %s' % error, file=sys.stderr)
    cg, overridden = cgroup_dir()
    before = read_enforcement(cg, overridden)
    if phases == PHASES[:4] and not enforcement_problems({**before, 'memory_peak': 1}, args.cap_bytes):
        code, timed_out = run_inner(cmd, args.timeout)
        receipt.update(exit=code, timed_out=timed_out, enforcement=read_enforcement(cg, overridden))
        phases.append('enforce')
        if code is not None:
            phases.append('command')
    else:
        receipt['enforcement'] = before
        print('lowmem-worker: not admitted (kernel cap or flock missing); the command was NOT run', file=sys.stderr)
    ctl = controller_cgroup()
    if ctl is not None:
        receipt['controller'] = read_enforcement(ctl, False)
    try:
        outputs = tree_hashes(out)
        if args.copy and Path(ROOT / args.copy).is_file():
            (out / 'result.json').write_bytes((ROOT / args.copy).read_bytes())
            outputs = tree_hashes(out)
        receipt['outputs'] = outputs
        receipt['output_bytes'] = {n: (out / n).stat().st_size for n in outputs}
        if outputs:
            phases.append('outputs')
    except (OSError, ValueError) as error:
        print('lowmem-worker: %s' % error, file=sys.stderr)
    after = swap_counters()
    receipt.update(seconds=round(time.monotonic() - start, 2), peak_bytes=(receipt['enforcement'] or {}).get('memory_peak'),
                   swap_total_bytes=meminfo().get('SwapTotal', 0), swap_before=swap_before, swap_after=after,
                   swap_activity=after != swap_before, mem_total_bytes=meminfo().get('MemTotal'),
                   boat_sandbox=os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == '1')
    (out / (args.id + '.receipt.json')).write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    why = receipt_problems(receipt, args.cap_bytes, require_boat=False)
    print('lowmem-worker: %s exit %s peak %s of %.2f GiB, %.0f s%s' % (
        args.id, receipt['exit'], ('%.2f GiB' % (receipt['peak_bytes'] / GIB)) if isinstance(receipt['peak_bytes'], int) else 'unknown',
        args.cap_bytes / GIB, receipt['seconds'], ' -- ' + '; '.join(why) if why else ''), flush=True)
    return 0 if not why else 1


def launcher(args):
    """Runs in the CONTROLLER scope (MemoryMax=CONTROLLER_CAP, swap 0), which stays alive to hold the chain:
    controller scope -> systemd-run compiler scope (3.7e9 decimal max, swap 0) -> heavy-guard <profile> -> worker -> command.
    The compiler scope is created before the guard so the guard's own-cgroup check is truthful; compiler + controller
    stay under 4 GiB. Pre-steps (proof partition `generate`) run here, outside the guard's exact-command contract."""
    py = str(Path(sys.executable).resolve())
    for pre in args.pre:
        step = shlex.split(pre)
        if subprocess.run(step, cwd=ROOT).returncode != 0:
            print('lowmem-launch: pre-step failed: %s' % pre, file=sys.stderr)
            return 1
    worker_argv = [py, str(ROOT / 'scripts/lowmem-build.py'), 'worker', '--id', args.id, '--out', args.out,
                   '--cap-bytes', str(args.cap_bytes), '--timeout', str(args.timeout),
                   *[x for p in args.protect for x in ('--protect', p)], *(['--copy', args.copy] if args.copy else []),
                   '--', *args.cmd]
    chain = systemd_scope(args.cap_bytes, ['--', py, str(ROOT / 'scripts/heavy-guard.py'), PROFILES[args.kind], '--', *worker_argv])
    try:
        return subprocess.run(chain, cwd=ROOT, timeout=args.timeout + 120).returncode
    except subprocess.TimeoutExpired:
        return 124


def receipt_problems(r, cap, require_boat=True):
    """Executor/cap/swap/exit problems of one worker receipt (shared with service-release.py's report check)."""
    if not isinstance(r, dict) or r.get('schema') != 3:
        return ['receipt is not a schema 3 worker receipt']
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
    t = r.get('timeout_seconds')
    if type(t) is not int or not 0 < t <= MAX_TIMEOUT:
        why.append('no finite timeout recorded')
    if r.get('timed_out') is not False:
        why.append('TIMED OUT or timeout not recorded')
    if r.get('flock') is not True or r.get('flock_path') != REAL_LOCK:
        why.append('native bend flock on %s was not held (no real admission)' % REAL_LOCK)
    if r.get('phases') != PHASES:
        why.append('incomplete phase set: %s' % r.get('phases'))
    if r.get('env') != PINNED_ENV:
        why.append('environment differs from the pinned environment')
    if not isinstance(r.get('protected'), dict) or set(r['protected']) != set(PROTECTED):
        why.append('protected bend.ts/safe.ts/base hashes missing')
    if not isinstance(r.get('tools'), dict) or not all(isinstance(r['tools'].get(k), dict) and r['tools'][k].get('sha256') for k in (('bend',) if str(r.get('id', '')).startswith('proof-') else ('bend', 'bun'))):
        why.append('compiler and Bun binary hashes required')
    if require_boat and (not r.get('boat_sandbox') or not isinstance(r.get('mem_total_bytes'), int) or r['mem_total_bytes'] > 8 * GIB):
        why.append('not a normal-size Boat sandbox')
    return why


def proof_jobs(proof_dir):
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
        # the guard runs ONLY the structured proof-shards.py run; the deterministic partition is regenerated beforehand
        pre = 'sh -c %s' % shlex.quote('[ -f %(d)s/manifest.json ] || python3 scripts/proof-shards.py generate --out %(d)s --shards %(n)d >/dev/null' % {'d': rel, 'n': count})
        jobs.append({'id': 'proof-' + s['id'], 'kind': 'proof', 'pre': [pre], 'copy': result, 'shard': s['id'], 'identity': identity,
                     'cmd': ['python3', 'scripts/proof-shards.py', 'run', '--out', rel, '--shard', s['id'], '--expect', identity, '--result', result]})
    return jobs


def plan(kinds, proof_dir):
    jobs = [{'id': k, 'kind': k, 'cmd': FIXED[k][0]} for k in ('hub', 'web', 'mobile') if k in kinds]
    if 'proof' in kinds:
        jobs += proof_jobs(proof_dir)
    return jobs


# Process hierarchy (agreed with lane 3, PR #854): the controller scope (512 MiB, swap 0) holds `launch`, which creates
# the compiler scope (3699998720 <= 3.7e9, swap 0) and runs heavy-guard <profile> in it; the guard takes the real bend
# flock, checks its own cgroup and execs the worker, which runs the one allowed command. 3699998720 + 536870912 < 4 GiB.
PROFILES = {'proof': 'proof-shard', 'hub': 'compile-shard', 'web': 'compile-shard', 'mobile': 'compile-shard'}


def launch_argv(job, out, timeout, cap, protect=()):
    """The launcher command line (it must itself run inside the controller scope)."""
    return ['python3', 'scripts/lowmem-build.py', 'launch', '--id', job['id'], '--kind', job['kind'], '--out', out,
            '--cap-bytes', str(cap), '--timeout', str(timeout), *[x for p in protect for x in ('--protect', p)],
            *(['--copy', job['copy']] if job.get('copy') else []), *[x for p in job.get('pre', []) for x in ('--pre', p)], '--', *job['cmd']]


def controller_argv(job, out, timeout, cap, protect=()):
    unit = 'lowmem-ctl-%s-%d' % (re.sub(r'[^A-Za-z0-9]', '-', job['id']), os.getpid())
    return systemd_scope(CONTROLLER_CAP, ['--unit=' + unit, '--', *launch_argv(job, out, timeout, cap, protect)])


def boat_argv(job, run_dir, timeout, protect=(), python=sys.executable):
    rel = 'build/lowmem/%s/%s' % (run_dir.name, job['id'])
    return [str(BOAT_JOB), '--size', 'default', '--timeout', str(timeout + 300), '--artifact', rel, '--',
            *controller_argv(job, rel, timeout, CAP_BYTES, protect)], rel


def protect_args(args):
    q = load_json(args.qualification) if args.qualification else None
    prot = (q or {}).get('protected') or {}
    return ['%s=%s' % (n, v['path']) for n, v in sorted(prot.items()) if isinstance(v, dict) and v.get('path')]


def launch(job, run_dir, args):
    """Run one job; returns (exit code, folder where its artifacts landed or None)."""
    if args.runner == 'local':
        out = run_dir / job['id']
        p = subprocess.run(controller_argv(job, str(out), args.timeout, args.cap_bytes, protect_args(args)), cwd=ROOT)
        return p.returncode, out
    argv, rel = boat_argv(job, run_dir, args.timeout, protect_args(args))
    log = run_dir / (job['id'] + '.log')
    with log.open('w') as handle:
        p = subprocess.run(argv, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT, text=True)
    m = re.search(r'logs and artifacts in (\S+)', log.read_text())
    return p.returncode, (Path(m.group(1)) / 'artifacts' / Path(rel).name if m else None)


def judge(job, receipt, reference, expected_source, qual, cap_bytes, proof=None, require_boat=True):
    """Every reason this job fails the gate (empty list = passed). qual is the qualification record."""
    if receipt is None:
        return ['no worker receipt (job never finished or artifact lost)']
    qual = qual if isinstance(qual, dict) else {}
    why = receipt_problems(receipt, cap_bytes, require_boat)
    if receipt.get('id') != job['id']:
        why.append('receipt belongs to %r' % receipt.get('id'))
    if receipt.get('source') != expected_source:
        why.append('source/import-closure mismatch: %s' % receipt.get('source'))
    elif not expected_source.get('clean'):
        why.append('source tree is not clean')
    qsrc = qual.get('source') or {}
    if any(qsrc.get(k) != expected_source.get(k) for k in ('tree', 'closure_sha256')):
        why.append('qualification names another source tree/import closure')
    tools = qual.get('tools')
    if not tools or {k: (receipt.get('tools') or {}).get(k) for k in ('bend', 'bun')} != {k: tools.get(k) for k in ('bend', 'bun')} and job['kind'] != 'proof':
        why.append('tool provenance mismatch with the qualified compiler/Bun: %s' % receipt.get('tools'))
    elif job['kind'] == 'proof' and (receipt.get('tools') or {}).get('bend') != (tools or {}).get('bend'):
        why.append('compiler mismatch with the qualification')
    if receipt.get('env') != qual.get('env'):
        why.append('environment/flags differ from the qualification')
    if receipt.get('protected') != {n: (v or {}).get('sha256') for n, v in (qual.get('protected') or {}).items()}:
        why.append('protected bend.ts/safe.ts/base differ from the qualification')
    if job['kind'] == 'proof':
        # a proof passes only on the planner's own result semantics; no verdict, no pass
        why.extend(proof if proof is not None else ['proof result was not validated'])
        return why
    spec = (qual.get('jobs') or {}).get(job['id']) or {}
    if receipt.get('cmd_template') != spec.get('cmd') or not spec.get('cmd'):
        why.append('command/flags differ from the qualification')
    if receipt.get('timeout_seconds') != spec.get('timeout_seconds'):
        why.append('timeout differs from the qualification')
    final = {n: {'sha256': h, 'bytes': (receipt.get('output_bytes') or {}).get(n)} for n, h in (receipt.get('outputs') or {}).items()}
    if final != spec.get('artifacts') or not final:
        why.append('final artifact names/sizes/SHA differ from the qualification')
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
    """(ok, reason, qualification). Bare {qualified:true} is not enough: it must name this exact commit, tree and
    import closure, the compiler and Bun binaries, the three protected inputs, the flags/environment, and each
    job's command, timeout and final artifact names, sizes and SHA."""
    q = load_json(path) if path else None
    if not isinstance(q, dict):
        return False, 'no readable qualification file', None
    if q.get('qualified') is not True or q.get('schema') != 3:
        return False, q.get('reason', 'compiler is not marked qualified (schema 3)'), None
    src, tools = q.get('source') or {}, q.get('tools') or {}
    if any(src.get(k) != source.get(k) for k in ('head', 'tree', 'closure_sha256')) or not source.get('clean'):
        return False, 'qualification is for another source commit/tree/import closure', None
    if not all(isinstance(tools.get(k), dict) and tools[k].get('sha256') for k in ('bend', 'bun')):
        return False, 'qualification does not name the compiler and Bun hashes', None
    prot = q.get('protected')
    if not isinstance(prot, dict) or set(prot) != set(PROTECTED) or not all(
            isinstance(v, dict) and v.get('path') and re.fullmatch(r'[0-9a-f]{64}', str(v.get('sha256', ''))) for v in prot.values()):
        return False, 'qualification must hash protected bend.ts, safe.ts and base', None
    if q.get('env') != PINNED_ENV:
        return False, 'qualification environment is not the pinned one', None
    jobs = q.get('jobs')
    if not isinstance(jobs, dict) or not {'hub', 'web', 'mobile'} <= set(jobs):
        return False, 'qualification lacks hub, web or mobile', None
    for name, spec in jobs.items():
        arts = spec.get('artifacts') if isinstance(spec, dict) else None
        if (not isinstance(arts, dict) or not arts or not spec.get('cmd') or spec.get('phases') != PHASES
                or type(spec.get('timeout_seconds')) is not int or not 0 < spec['timeout_seconds'] <= MAX_TIMEOUT
                or not all(isinstance(a, dict) and re.fullmatch(r'[0-9a-f]{64}', str(a.get('sha256', ''))) and type(a.get('bytes')) is int and a['bytes'] > 0
                           for a in arts.values())):
            return False, 'qualification job %s lacks command, timeout, phases or final artifact size/SHA' % name, None
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
                print('  %-22s %s' % ('', shlex.join(boat_argv(j, run_dir, args.timeout, protect_args(args))[0])))
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
    if (not isinstance(reference, dict) or reference.get('schema') != 2 or not reference.get('producer')
            or any((reference.get('source') or {}).get(k) != source[k] for k in ('head', 'tree'))):
        report.update(state='unqualified', reason='reference emit is missing, not from a verified producer, or for another commit/tree')
        return unqualified(args, report, report['reason'])
    missing = sorted(k for k in kinds & {'hub', 'web', 'mobile'} if not (reference.get('outputs') or {}).get(k))
    if missing:  # no reference, no comparison: refuse before spending any job (the frozen producer has no mobile output)
        report.update(state='unqualified', reason='reference emit has no output for: ' + ', '.join(missing))
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
        why = judge(j, receipt, reference, source, qual, args.cap_bytes,
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
    report.update(source=source, tools=qual['tools'], protected={n: v['sha256'] for n, v in qual['protected'].items()}, cap_bytes=args.cap_bytes, release=bool(args.release),
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


def reference_from_emission(d, expect_server_sha=None):
    """The reference emit comes only from a saved, verified official producer (EMISSION.json + inventory), never
    from bytes a caller picks: its exact commit/tree, compiler, phases and every file hash are checked."""
    d = Path(d)
    e = json.loads((d / 'EMISSION.json').read_text())
    if not (e.get('schema') == 1 and e.get('kind') == 'backplane-secret-free-js-emission' and e.get('status') == 'VERIFIED'
            and e.get('channel') == 'release' and e.get('published') is False
            and re.fullmatch(r'[0-9a-f]{40}', str(e.get('sourceCommit'))) and re.fullmatch(r'[0-9a-f]{40}', str(e.get('sourceTree')))):
        raise ValueError('not a verified producer emission')
    actual = {}
    for p in sorted(d.rglob('*')):
        if p.is_symlink():
            raise ValueError('symlink in producer emission')
        if p.is_file() and p.name != 'EMISSION.json':
            actual[p.relative_to(d).as_posix()] = {'sha256': sha(p.read_bytes()), 'bytes': p.stat().st_size}
    if actual != e.get('files'):
        raise ValueError('producer emission member/hash mismatch')
    src = json.loads((d / 'SOURCE.json').read_text())
    if src.get('commit') != e['sourceCommit'] or src.get('tree', e['sourceTree']) != e['sourceTree']:
        raise ValueError('producer source manifest differs from its emission')
    phases = json.loads((d / 'PHASES.json').read_text())
    if [x.get('phase') for x in phases] != ['entries', 'server', 'web'] or any(x.get('exitCode') != 0 for x in phases):
        raise ValueError('producer phases incomplete or failed')
    server = actual['server.js']['sha256']
    if expect_server_sha and server != expect_server_sha:
        raise ValueError('producer server.js is not the pinned %s' % expect_server_sha)
    web = {'web/' + k[len('web/'):]: v['sha256'] for k, v in actual.items() if k.startswith('web/')}
    if not web:
        raise ValueError('producer has no web output')
    return {'schema': 2, 'source': {'head': e['sourceCommit'], 'tree': e['sourceTree']},
            'producer': {'emission_sha256': sha((d / 'EMISSION.json').read_bytes()), 'compiler': e['compiler'],
                         'emitterSha256': e.get('emitterSha256'), 'serverEmitKey': e.get('serverEmitKey'),
                         'phases': [x['phase'] for x in phases], 'files': actual},
            'outputs': {'hub': {'hub.js': server}, 'web': web}}


def make_reference(args):
    ref = reference_from_emission(args.from_emission, args.expect_server_sha)
    Path(args.out).write_text(json.dumps(ref, indent=2, sort_keys=True) + '\n')
    print('reference from producer %s written to %s (hub and web only: no mobile producer is available)' % (
        ref['source']['head'][:12], args.out))
    return 0


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
    f.add_argument('--from-emission', required=True)
    f.add_argument('--expect-server-sha')
    f.add_argument('--out', required=True)
    w = sub.add_parser('worker')
    w.add_argument('--id', required=True)
    w.add_argument('--out', required=True)
    w.add_argument('--cap-bytes', type=int, default=CAP_BYTES)
    w.add_argument('--copy')
    w.add_argument('--timeout', type=int, default=3000)
    w.add_argument('--protect', action='append', default=[], help='NAME=PATH of bend.ts, safe.ts, base')
    w.add_argument('cmd', nargs=argparse.REMAINDER)
    h = sub.add_parser('launch')
    h.add_argument('--id', required=True)
    h.add_argument('--kind', required=True, choices=sorted(PROFILES))
    h.add_argument('--out', required=True)
    h.add_argument('--cap-bytes', type=int, default=CAP_BYTES)
    h.add_argument('--timeout', type=int, default=3000)
    h.add_argument('--protect', action='append', default=[])
    h.add_argument('--copy')
    h.add_argument('--pre', action='append', default=[])
    h.add_argument('cmd', nargs=argparse.REMAINDER)
    args = p.parse_args(argv)
    if args.op == 'launch':
        if args.cmd[:1] == ['--']:
            args.cmd = args.cmd[1:]
        return launcher(args) if args.cmd else 2
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
