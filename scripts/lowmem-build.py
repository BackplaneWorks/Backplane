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
CAP_BYTES = int(3.7 * GIB)
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


def tree_rss(root_pid):
    """Sum of RSS in bytes over root_pid and its descendants."""
    kids, rss = {}, {}
    for d in os.listdir('/proc'):
        if not d.isdigit():
            continue
        try:
            stat = Path('/proc/%s/stat' % d).read_text()
            ppid = int(stat.rsplit(')', 1)[1].split()[1])
            kids.setdefault(ppid, []).append(int(d))
            for line in Path('/proc/%s/status' % d).read_text().splitlines():
                if line.startswith('VmRSS:'):
                    rss[int(d)] = int(line.split()[1]) * 1024
        except (OSError, ValueError, IndexError):
            continue
    total, stack = 0, [root_pid]
    while stack:
        p = stack.pop()
        total += rss.get(p, 0)
        stack.extend(kids.get(p, []))
    return total


def cgroup_peak():
    try:
        group = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
        return int((Path('/sys/fs/cgroup') / group / 'memory.peak').read_text())
    except (OSError, IndexError, ValueError):
        return None


def worker(args):
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    cmd = [c.replace('{out}', str(out)) for c in args.cmd]
    swap_total = meminfo().get('SwapTotal', 0)
    before = swap_counters()
    start = time.monotonic()
    proc = subprocess.Popen(cmd, cwd=ROOT, start_new_session=True)
    peak, over, status = 0, False, None
    while status is None:
        pid, st, ru = os.wait4(proc.pid, os.WNOHANG)
        if pid:
            status = st
            # a lower bound for the largest single process, so a short job still records a peak
            peak = max(peak, ru.ru_maxrss * 1024)
            break
        peak = max(peak, tree_rss(proc.pid))
        if peak > args.cap_bytes:
            over = True
            os.killpg(proc.pid, signal.SIGKILL)
            _, status, ru = os.wait4(proc.pid, 0)
            break
        time.sleep(args.poll)
    proc.returncode = os.waitstatus_to_exitcode(status)
    if peak > args.cap_bytes:
        over = True
    seconds = time.monotonic() - start
    cg = cgroup_peak()
    # The sandbox cgroup also holds the harness; its peak is a second witness, never a relaxation.
    if cg is not None and cg > args.cap_bytes and args.cgroup_binding:
        over = True
    after = swap_counters()
    swapped = after != before
    try:
        outputs = tree_hashes(out) if out.exists() else {}
    except ValueError as error:
        outputs = {}
        print('lowmem-worker: %s' % error, file=sys.stderr)
    if args.copy and Path(ROOT / args.copy).is_file():
        (out / 'result.json').write_bytes((ROOT / args.copy).read_bytes())
        outputs = tree_hashes(out)
    receipt = {'schema': 1, 'id': args.id, 'cmd': cmd, 'exit': proc.returncode, 'seconds': round(seconds, 2),
               'cap_bytes': args.cap_bytes, 'peak_bytes': max(peak, 0), 'cgroup_peak_bytes': cg,
               'over_cap': over, 'swap_total_bytes': swap_total, 'swap_before': before, 'swap_after': after,
               'swap_activity': swapped, 'mem_total_bytes': meminfo().get('MemTotal'),
               'source': source_identity(), 'tools': tool_identity(), 'outputs': outputs}
    (out / (args.id + '.receipt.json')).write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    ok = proc.returncode == 0 and not over and not swapped and bool(outputs)
    print('lowmem-worker: %s exit %s peak %.2f GiB of %.2f, %.0f s%s' % (
        args.id, proc.returncode, peak / GIB, args.cap_bytes / GIB, seconds,
        ' OVER CAP' if over else (' SWAP ACTIVITY' if swapped else '')), flush=True)
    return 0 if ok else 1


# ---------------------------------------------------------------- orchestrator

PROOF_WRAPPER = ['python3', 'scripts/heavy-guard.py', 'proof', '--']  # lane 3 may supply a shard-sized profile


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
        jobs.append({'id': 'proof-' + s['id'], 'kind': 'proof', 'cmd': ['sh', '-c', script], 'copy': result})
    return jobs


def plan(kinds, proof_dir):
    jobs = [{'id': k, 'kind': k, 'cmd': FIXED[k][0]} for k in ('hub', 'web', 'mobile') if k in kinds]
    if 'proof' in kinds:
        jobs += proof_jobs(proof_dir)
    return jobs


def boat_argv(job, run_dir, timeout, python=sys.executable):
    rel = 'build/lowmem/%s/%s' % (run_dir.name, job['id'])
    worker_cmd = ['python3', 'scripts/lowmem-build.py', 'worker', '--id', job['id'], '--out', rel,
                  '--cap-bytes', str(CAP_BYTES), '--cgroup-binding', *(['--copy', job['copy']] if job.get('copy') else []), '--', *[c.replace('{out}', rel) for c in job['cmd']]]
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


def judge(job, receipt, reference, expected_source, expected_tools, cap_bytes):
    """Every reason this job fails the gate (empty list = passed)."""
    why = []
    if receipt is None:
        return ['no worker receipt (job never finished or artifact lost)']
    if receipt.get('id') != job['id']:
        why.append('receipt belongs to %r' % receipt.get('id'))
    if receipt.get('exit') != 0:
        why.append('worker exit %s' % receipt.get('exit'))
    if receipt.get('cap_bytes') != cap_bytes:
        why.append('cap %s is not the required %s' % (receipt.get('cap_bytes'), cap_bytes))
    if receipt.get('over_cap') or (receipt.get('peak_bytes') or 0) > cap_bytes:
        why.append('OVER CAP: peak %.2f GiB > %.2f GiB' % ((receipt.get('peak_bytes') or 0) / GIB, cap_bytes / GIB))
    if receipt.get('swap_activity'):
        why.append('SWAP used (kernel swap counters moved)')
    if receipt.get('source') != expected_source:
        why.append('source mismatch: %s' % receipt.get('source'))
    elif not expected_source.get('clean'):
        why.append('source tree is not clean')
    if expected_tools and receipt.get('tools') != expected_tools:
        why.append('tool provenance mismatch: %s' % receipt.get('tools'))
    if not receipt.get('tools') or not receipt['tools'].get('bend'):
        why.append('no bend tool provenance')
    if job['kind'] == 'proof':
        if not receipt.get('outputs'):
            why.append('no proof receipt output')
        return why
    want = (reference or {}).get('outputs', {}).get(job['id'])
    if want is None:
        why.append('no reference emit for %s' % job['id'])
    elif receipt.get('outputs') != want:
        diff = sorted(set(want) ^ set(receipt.get('outputs') or {}) |
                      {k for k in want if k in (receipt.get('outputs') or {}) and want[k] != receipt['outputs'][k]})
        why.append('NOT BYTE-IDENTICAL to reference: %s' % ', '.join(diff[:8]))
    return why


def load_json(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None


def qualified(path):
    q = load_json(path) if path else None
    if not isinstance(q, dict):
        return False, 'no readable qualification file'
    if q.get('qualified') is not True:
        return False, q.get('reason', 'compiler is not marked qualified')
    return True, ''


def banner(text):
    line = '!' * 72
    print('\n%s\n%s\n%s\n' % (line, text, line), file=sys.stderr, flush=True)


def run(args):
    kinds = set(args.jobs.split(','))
    if not kinds <= {'hub', 'web', 'mobile', 'proof'}:
        print('unknown job kind', file=sys.stderr)
        return 2
    jobs = plan(kinds, args.proof_dir)
    ok, reason = qualified(args.qualification)
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
    report = {'schema': 1, 'started': stamp, 'runner': args.runner, 'qualified': ok, 'fallback': False, 'jobs': {}}
    if args.release and args.runner != 'boat':
        print('--release needs the boat runner', file=sys.stderr)
        return 2
    if not ok or not jobs:
        reason = reason or 'no jobs planned (proof manifest missing?)'
        report.update(state='unqualified', reason=reason)
        return unqualified(args, report, reason)
    run_dir.mkdir(parents=True, exist_ok=True)
    reference = load_json(args.reference) if args.reference else None
    source = source_identity()
    tools = (reference or {}).get('tools')
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
    failed = {}
    for j in jobs:
        rc, receipt, folder = results[j['id']]
        why = judge(j, receipt, reference, source, tools, args.cap_bytes)
        if rc != 0 and not why:
            why.append('boat-job exit %s' % rc)
        entry = report['jobs'].setdefault(j['id'], {})
        entry.update(exit=rc, seconds=(receipt or {}).get('seconds'), peak_bytes=(receipt or {}).get('peak_bytes'),
                     ok=not why, why=why)
        if why:
            failed[j['id']] = why
    report.update(wall_seconds=round(wall, 1), state='failed' if failed else 'passed',
                  peak_bytes=max([e.get('peak_bytes') or 0 for e in report['jobs'].values()] or [0]))
    write_report(run_dir, report)
    print('\nend-to-end wall %.0f s, %d jobs, peak per job:' % (wall, len(jobs)))
    for k, e in sorted(report['jobs'].items()):
        print('  %-22s %s  peak %s  %s' % (k, 'ok  ' if e.get('ok') else 'FAIL',
              '%.2f GiB' % (e['peak_bytes'] / GIB) if e.get('peak_bytes') else 'n/a', '; '.join(e.get('why', []))))
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
    w.add_argument('--cgroup-binding', action='store_true')
    w.add_argument('--copy')
    w.add_argument('cmd', nargs=argparse.REMAINDER)
    args = p.parse_args(argv)
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
