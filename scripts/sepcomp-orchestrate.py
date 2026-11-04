#!/usr/bin/env python3
"""PROVISIONAL, OFFLINE scaffold for separate-compilation orchestration (lane 6). Nothing here compiles anything.

  contract   sepcomp-provisional-v0   -- NOT a claim that any interface, object or linker format is sound.
  workers    FAKE deterministic workers only; no Boat, no guard, no release/CI wiring, no real compiler.
  real perf  UNMEASURED.

What is real here is the part that does not depend on the compiler: re-deriving the complete unit/SCC inventory and
dependency graph from source, a frozen plan hash, a dependency-ready scheduler (no wave barriers) that is deterministic
in what it records, and an aggregator that trusts no worker: it recomputes the plan, every artifact hash, the
interface-hash chain, the link inputs and order, and compares the link output with an independent reference.

  scripts/sepcomp-orchestrate.py plan     [--root R]            print the inventory and plan hash
  scripts/sepcomp-orchestrate.py dry-run  [--root R] [--parallel N]  simulate the dependency-ready schedule, launch nothing
  scripts/sepcomp-orchestrate.py fake-run --out DIR [--root R]  run FAKE workers locally, write receipts, then verify
  scripts/sepcomp-orchestrate.py verify   --run DIR [--root R]  re-derive and check a run directory
"""
import argparse
import concurrent.futures
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTRACT = 'sepcomp-provisional-v0'
ENTRY_ROOTS = ['src/server/main.bend', 'src/web/app.bend', 'src/mobile/app.bend']
BEND_IMPORT = re.compile(r'^\s*import\s+(\S+)(?:\s+as\s+[A-Za-z_][A-Za-z0-9_]*)?\s*(?:#.*)?$')
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"')
COMPILER_CAP, CONTROLLER_CAP = 3699998720, 536870912   # the #853 caps; their sum stays under 4 GiB
GIB = 1024 ** 3
ARTIFACTS = ('interface.bin', 'object.bin', 'verdict.json')
RECEIPT_KEYS = {'contract', 'plan_sha256', 'unit', 'members', 'static_id', 'dynamic_key', 'deps', 'tools', 'caps',
                'status', 'exit', 'worker_kind', 'sources', 'artifacts'}


class PlanError(ValueError):
    pass


def sha(data):
    return hashlib.sha256(data if isinstance(data, bytes) else data.encode()).hexdigest()


def canon(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


# ------------------------------------------------------------------ inventory (fail closed, like bend-closure.py)

def rel(path):
    return os.path.relpath(os.path.normpath(path))


def parse_file(root, f):
    """(bend module imports, companion files) of one file, using bend-closure.py's grammar. Fails closed."""
    if f.startswith('..') or os.path.isabs(f):
        raise PlanError('outside the checkout: ' + f)
    try:
        text = (root / f).read_text(encoding='utf-8')
    except OSError as e:
        raise PlanError('cannot read %s: %s' % (f, e))
    here = os.path.dirname(f)
    mods, comps = [], []
    if f.endswith('.bend'):
        for line in text.split('\n'):
            if not re.match(r'^\s*import\b', line):
                continue
            m = BEND_IMPORT.match(line)
            if not m:
                raise PlanError('%s: an import it does not understand: %r' % (f, line))
            name = m.group(1)
            if name.startswith('"') and name.endswith('"'):
                name = name[1:-1]
            if name.startswith(('./', '../')):
                (mods if name.endswith('.bend') else comps).append(rel(os.path.join(here, name)))
            elif not re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', name):
                raise PlanError('%s: an import it does not understand: %r' % (f, line))
    elif f.endswith(('.c', '.h')):
        for line in text.split('\n'):
            m = INCLUDE.match(line)
            if m and (root / rel(os.path.join(here, m.group(1)))).exists():
                comps.append(rel(os.path.join(here, m.group(1))))
    return mods, comps


def inventory(root, roots=ENTRY_ROOTS):
    """modules {path: [imported module paths]}, companions {module: sorted companion files (transitive)}."""
    modules, companions, seen, todo = {}, {}, set(), [rel(r) for r in roots]
    while todo:
        f = todo.pop()
        if f in modules:
            continue
        mods, comps = parse_file(root, f)
        modules[f] = sorted(set(mods))
        own, stack = set(), list(comps)
        while stack:
            c = stack.pop()
            if c in own:
                continue
            own.add(c)
            stack.extend(parse_file(root, c)[1])
        companions[f] = sorted(own)
        todo.extend(mods)
    for deps in modules.values():
        for d in deps:
            if d not in modules:
                raise PlanError('unresolved module ' + d)
    return modules, companions


def sccs(graph):
    """Iterative Tarjan; deterministic (sorted starts and successors). Returns lists of members, each sorted."""
    index, low, on, stack, out, counter = {}, {}, set(), [], [], [0]
    for start in sorted(graph):
        if start in index:
            continue
        work = [(start, iter(sorted(graph[start])))]
        index[start] = low[start] = counter[0]; counter[0] += 1; stack.append(start); on.add(start)
        while work:
            v, it = work[-1]
            advanced = False
            for w in it:
                if w not in index:
                    index[w] = low[w] = counter[0]; counter[0] += 1; stack.append(w); on.add(w)
                    work.append((w, iter(sorted(graph[w]))))
                    advanced = True
                    break
                if w in on:
                    low[v] = min(low[v], index[w])
            if advanced:
                continue
            work.pop()
            if work:
                low[work[-1][0]] = min(low[work[-1][0]], low[v])
            if low[v] == index[v]:
                comp = []
                while True:
                    w = stack.pop(); on.discard(w); comp.append(w)
                    if w == v:
                        break
                out.append(sorted(comp))
    return sorted(out)


def make_plan(root, tools, roots=ENTRY_ROOTS):
    """The frozen plan. tools = {'compiler_sha256','base_sha256','options','env'} (FAKE values in the scaffold).
    Units are SCCs (atomic). Everything is derived from source bytes; plan_sha256 covers all of it."""
    root = Path(root)
    modules, companions = inventory(root, roots)
    comps = sccs(modules)
    owner = {m: i for i, c in enumerate(comps) for m in c}
    units = {}
    for i, members in enumerate(comps):
        uid = sha(canon(members))[:16]
        files = sorted(set(members) | {c for m in members for c in companions[m]})
        src = {f: sha((root / f).read_bytes()) for f in files}
        deps = sorted({sha(canon(comps[owner[d]]))[:16] for m in members for d in modules[m] if owner[d] != i})
        static = sha(canon({'contract': CONTRACT, 'members': members, 'sources': src, 'tools': tools}))
        units[uid] = {'unit': uid, 'members': members, 'deps': deps, 'sources': src, 'static_id': static}
    order = topo_order(units)
    body = {'contract': CONTRACT, 'tools': tools, 'roots': sorted(rel(r) for r in roots), 'units': units, 'order': order}
    return {**body, 'plan_sha256': sha(canon(body))}


def topo_order(units):
    """Kahn with ties by unit id: a canonical order independent of any run."""
    indeg = {u: len(v['deps']) for u, v in units.items()}
    users = {u: [] for u in units}
    for u, v in units.items():
        for d in v['deps']:
            users[d].append(u)
    ready, out = sorted(u for u, n in indeg.items() if n == 0), []
    while ready:
        u = ready.pop(0)
        out.append(u)
        for x in users[u]:
            indeg[x] -= 1
            if indeg[x] == 0:
                ready = sorted(ready + [x])
    if len(out) != len(units):
        raise PlanError('condensation is not acyclic')  # cannot happen after SCC condensation: a bug, fail closed
    return out


def levels(plan):
    lv = {}
    for u in plan['order']:
        lv[u] = 1 + max((lv[d] for d in plan['units'][u]['deps']), default=-1)
    return lv


# ------------------------------------------------------------------ the PROVISIONAL receipt / interface hashing

def dynamic_key(unit, dep_interfaces):
    """Static identity + the ordered direct-dependency interface hashes. Ordering is by dep unit id."""
    return sha(canon({'static_id': unit['static_id'], 'deps': [[d, dep_interfaces[d]] for d in unit['deps']]}))


def caps_record(kind):
    return {'compiler_cap': COMPILER_CAP, 'controller_cap': CONTROLLER_CAP, 'swap_max': 0, 'enforcement': kind}


# ------------------------------------------------------------------ FAKE worker (labelled; deterministic; fault injection)

class FakeWorker:
    """Stands in for `unit-compile`. The 'compiler' is a hash function over the inputs, so a correct run is
    reproducible by an independent whole-program reference (reference_output). Never evidence about a real compiler."""
    kind = 'FAKE-DETERMINISTIC'

    def __init__(self, faults=None, delay=None):
        self.faults, self.delay, self.calls = faults or {}, delay, []
        self.lock = threading.Lock()

    def __call__(self, plan, unit, dep_interfaces, out):
        with self.lock:
            self.calls.append(unit['unit'])
        if self.delay:
            time.sleep(self.delay(unit['unit']))
        fault = self.faults.get(unit['unit'])
        if fault == 'defer':
            raise Deferred('fake admission refusal')
        iface = sha(canon(['iface', unit['static_id'], [dep_interfaces[d] for d in unit['deps']]]))
        obj = sha(canon(['obj', unit['static_id'], iface]))
        out.mkdir(parents=True, exist_ok=True)
        (out / 'interface.bin').write_text(iface)
        (out / 'object.bin').write_text(obj)
        failed = fault == 'fail'
        (out / 'verdict.json').write_text(json.dumps({'verdict': 'rejected' if failed else 'accepted'}))
        receipt = {'contract': CONTRACT, 'plan_sha256': plan['plan_sha256'], 'unit': unit['unit'], 'members': unit['members'],
                   'static_id': unit['static_id'], 'dynamic_key': dynamic_key(unit, dep_interfaces),
                   'deps': [[d, dep_interfaces[d]] for d in unit['deps']], 'tools': plan['tools'], 'caps': caps_record('FAKE'),
                   'status': 'failed' if failed else 'passed', 'exit': 1 if failed else 0, 'worker_kind': self.kind,
                   'sources': unit['sources'], 'artifacts': {n: {'sha256': sha((out / n).read_bytes()), 'bytes': (out / n).stat().st_size}
                                                              for n in ARTIFACTS}}
        if fault == 'forge-hash':
            receipt['artifacts']['object.bin']['sha256'] = sha(b'forged')
        if fault == 'false-pass':                        # claims passed while the verdict file says rejected
            (out / 'verdict.json').write_text(json.dumps({'verdict': 'rejected'}))
            receipt['artifacts']['verdict.json'] = {'sha256': sha((out / 'verdict.json').read_bytes()),
                                                   'bytes': (out / 'verdict.json').stat().st_size}
        if fault == 'wrong-deps':
            receipt['deps'] = [[d, sha(b'stale')] for d in unit['deps']]
        if fault == 'extra-key':
            receipt['surprise'] = 1
        (out / 'receipt.json').write_text(json.dumps(receipt, sort_keys=True))
        return receipt


class FakeLinker:
    kind = 'FAKE-DETERMINISTIC'

    def __call__(self, plan, ordered, out, swap=None):
        order = [[u, ordered[u]] for u in plan['order']] if swap is None else swap
        out.mkdir(parents=True, exist_ok=True)
        data = sha(canon(['link', [o for _, o in order]]))
        (out / 'program.bin').write_text(data)
        receipt = {'contract': CONTRACT, 'plan_sha256': plan['plan_sha256'], 'inputs': order, 'tools': plan['tools'],
                   'caps': caps_record('FAKE'), 'worker_kind': self.kind, 'status': 'passed', 'exit': 0,
                   'output': {'sha256': sha(data), 'bytes': len(data)}}
        (out / 'link.json').write_text(json.dumps(receipt, sort_keys=True))
        return receipt


def reference_output(plan):
    """The independent 'stock whole-program' stand-in: recomputes the fake semantics from the plan alone, with no
    receipts and no per-unit directories. A real reference is a saved verified same-source producer emission."""
    iface_hash, obj_hash = {}, {}
    for u in plan['order']:
        unit = plan['units'][u]
        iface = sha(canon(['iface', unit['static_id'], [iface_hash[d] for d in unit['deps']]]))
        obj = sha(canon(['obj', unit['static_id'], iface]))
        iface_hash[u], obj_hash[u] = sha(iface), sha(obj)         # what receipts and link inputs carry: hashes of the files
    return sha(sha(canon(['link', [obj_hash[u] for u in plan['order']]])))


# ------------------------------------------------------------------ dependency-ready scheduler (no wave barriers)

class Deferred(Exception):
    """Admission refused by the real account limits (boat-job exit 75-like). Never a reason to use a larger machine."""


class FixedAdmission:
    """Stand-in for the real limits: at most `active` concurrent jobs, `starts` total attempts per call to start()."""

    def __init__(self, active):
        self.active = active


def schedule(plan, worker, run_dir, parallel=4, max_deferrals=5, backoff=0.0):
    """Dependency-ready: a unit starts as soon as all its deps passed; no level barriers. Deterministic records:
    submission picks the smallest ready unit id; results are keyed by unit id and the link order is canonical, so
    nothing recorded depends on completion order. First failure stops new submissions (fail-fast); running units
    finish. Admission refusals (Deferred) are retried a bounded number of times, then count as failure."""
    units, done, failed, running, deferrals = plan['units'], {}, {}, {}, {}
    pending = set(units)
    iface = {}
    started = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, parallel)) as pool:
        while pending or running:
            ready = sorted(u for u in pending if all(d in done for d in units[u]['deps']))
            while ready and len(running) < parallel and not failed:
                u = ready.pop(0)
                pending.discard(u)
                started.append(u)
                deps = {d: iface[d] for d in units[u]['deps']}
                running[pool.submit(worker, plan, units[u], deps, Path(run_dir) / 'units' / u)] = u
            if not running:
                break
            finished, _ = concurrent.futures.wait(running, return_when=concurrent.futures.FIRST_COMPLETED)
            for f in sorted(finished, key=lambda x: running[x]):
                u = running.pop(f)
                try:
                    r = f.result()
                except Deferred:
                    deferrals[u] = deferrals.get(u, 0) + 1
                    if deferrals[u] > max_deferrals:
                        failed[u] = 'admission refused too often'
                    else:
                        pending.add(u)
                        started.remove(u)
                        time.sleep(backoff)
                    continue
                except Exception as e:  # a worker that cannot even finish is a failure, not a pass
                    failed[u] = 'worker error: %s' % e
                    continue
                if r.get('status') != 'passed':
                    failed[u] = 'unit rejected'
                else:
                    done[u] = r
                    iface[u] = json.loads((Path(run_dir) / 'units' / u / 'receipt.json').read_text())['artifacts']['interface.bin']['sha256']
    return {'passed': not failed and len(done) == len(units), 'failed': failed, 'done': sorted(done),
            'not_run': sorted(set(units) - set(done) - set(failed)), 'deferrals': deferrals}


def simulate(plan, parallel):
    """Dry-run: unit costs unknown, so assume 1 unit time each. Shows structure, not performance (UNMEASURED)."""
    lv = levels(plan)
    done, t, timeline = set(), 0, []
    pending = set(plan['units'])
    while pending:
        ready = sorted(u for u in pending if all(d in done for d in plan['units'][u]['deps']))
        batch = ready[:parallel]
        timeline.append(batch)
        done.update(batch)
        pending.difference_update(batch)
    return {'units': len(plan['units']), 'cyclic_units': sum(len(v['members']) > 1 for v in plan['units'].values()),
            'levels': max(lv.values()) + 1 if lv else 0, 'max_width': max((list(lv.values()).count(i) for i in set(lv.values())), default=0),
            'dependency_ready_steps_at_unit_cost': len(timeline), 'parallel': parallel,
            'note': 'structure only; real per-unit time, blob sizes and Boat overhead are UNMEASURED'}


# ------------------------------------------------------------------ aggregator: trusts nothing a worker wrote

def check(ok, why, errors):
    if not ok:
        errors.append(why)
    return ok


def verify_run(root, run_dir, tools, plan_file=None, allow_fake=False, roots=ENTRY_ROOTS):
    """Re-derive everything from the source tree and the run directory. Returns a list of reasons (empty = verified)."""
    err = []
    run = Path(run_dir)
    try:
        plan = make_plan(root, tools, roots)          # recomputed now: catches post-plan source mutation
    except PlanError as e:
        return ['plan cannot be re-derived: %s' % e]
    if plan_file is not None:
        frozen = json.loads(Path(plan_file).read_text())
        check(frozen == plan, 'POST-PLAN MUTATION: source/tools differ from the frozen plan (%s vs %s)' % (
            frozen.get('plan_sha256', '')[:12], plan['plan_sha256'][:12]), err)
        if frozen != plan:
            return err
    units = plan['units']
    # exact file set: nothing extra, nothing missing
    expected = {'link/link.json', 'link/program.bin', 'plan.json'}
    for u in units:
        expected |= {'units/%s/%s' % (u, n) for n in (*ARTIFACTS, 'receipt.json')}
    actual = {p.relative_to(run).as_posix() for p in run.rglob('*') if p.is_file()} if run.is_dir() else set()
    check(not actual - expected, 'extra artifacts: %s' % sorted(actual - expected)[:5], err)
    check(not expected - actual, 'missing artifacts: %s' % sorted(expected - actual)[:5], err)
    if err:
        return err
    iface, objs = {}, {}
    for u in plan['order']:
        unit, d = units[u], run / 'units' / u
        r = json.loads((d / 'receipt.json').read_text())
        pre = len(err)
        check(set(r) == RECEIPT_KEYS, '%s: receipt keys differ from the contract' % u, err)
        check(r.get('contract') == CONTRACT and r.get('plan_sha256') == plan['plan_sha256'], '%s: receipt is for another plan/contract' % u, err)
        check(r.get('unit') == u and r.get('members') == unit['members'], '%s: SCC/members differ from the plan (atomicity)' % u, err)
        check(r.get('static_id') == unit['static_id'] and r.get('sources') == unit['sources'], '%s: source identity differs' % u, err)
        check(r.get('tools') == plan['tools'], '%s: tool/options identity differs' % u, err)
        c = r.get('caps') or {}
        check(c.get('compiler_cap') == COMPILER_CAP and c.get('controller_cap') == CONTROLLER_CAP and c.get('swap_max') == 0
              and COMPILER_CAP + CONTROLLER_CAP < 4 * GIB, '%s: caps differ' % u, err)
        check(allow_fake or (r.get('worker_kind') != 'FAKE-DETERMINISTIC' and c.get('enforcement') != 'FAKE'),
              '%s: FAKE worker evidence is not accepted here' % u, err)
        check(r.get('status') == 'passed' and r.get('exit') == 0, '%s: unit did not pass' % u, err)
        for n in ARTIFACTS:                                # artifacts recomputed from bytes, never from the receipt
            data = (d / n).read_bytes()
            a = (r.get('artifacts') or {}).get(n) or {}
            check(a.get('sha256') == sha(data) and a.get('bytes') == len(data), '%s: %s hash/size does not match the bytes' % (u, n), err)
        try:
            verdict = json.loads((d / 'verdict.json').read_text()).get('verdict')
        except ValueError:
            verdict = None
        check(verdict == 'accepted', '%s: verdict file is %r (false-pass control)' % (u, verdict), err)
        if all(x in iface for x in unit['deps']):
            want = [[x, iface[x]] for x in unit['deps']]
            check(r.get('deps') == want, '%s: consumed dependency interfaces are not the upstream receipts\' (stale/forged chain)' % u, err)
            check(r.get('dynamic_key') == dynamic_key(unit, {x: iface[x] for x in unit['deps']}), '%s: dynamic key mismatch' % u, err)
        else:
            err.append('%s: a dependency has no verified interface' % u)
        if len(err) == pre:                               # only a fully verified unit may feed its consumers
            iface[u] = sha((d / 'interface.bin').read_bytes())
            objs[u] = sha((d / 'object.bin').read_bytes())
    link = json.loads((run / 'link/link.json').read_text())
    want_inputs = [[u, objs[u]] for u in plan['order'] if u in objs]
    check(link.get('plan_sha256') == plan['plan_sha256'], 'link: another plan', err)
    check(link.get('inputs') == want_inputs and len(want_inputs) == len(units),
          'link: inputs are not exactly the verified objects in canonical order (missing/duplicate/reordered)', err)
    check(link.get('tools') == plan['tools'] and link.get('status') == 'passed' and link.get('exit') == 0, 'link: tools/status', err)
    check(allow_fake or link.get('worker_kind') != 'FAKE-DETERMINISTIC', 'link: FAKE linker evidence is not accepted here', err)
    prog = (run / 'link/program.bin').read_bytes()
    check((link.get('output') or {}) == {'sha256': sha(prog), 'bytes': len(prog)}, 'link: output hash does not match its bytes', err)
    check(sha(prog) == reference_output(plan), 'BYTES DIFFER from the independent reference', err)
    return err


# ------------------------------------------------------------------ CLI

FAKE_TOOLS = {'compiler_sha256': 'f' * 64, 'base_sha256': 'e' * 64, 'options': [], 'env': {'BEND_NO_TELEMETRY': '1'},
              'note': 'FAKE placeholder identities'}


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('op', choices=('plan', 'dry-run', 'fake-run', 'verify'))
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--parallel', type=int, default=8)
    p.add_argument('--out', type=Path)
    p.add_argument('--run', type=Path)
    a = p.parse_args(argv)
    try:
        plan = make_plan(a.root, FAKE_TOOLS)
    except PlanError as e:
        print('sepcomp: plan failed closed: %s' % e, file=sys.stderr)
        return 1
    if a.op == 'plan':
        print(json.dumps({'plan_sha256': plan['plan_sha256'], **simulate(plan, a.parallel)}, indent=2))
        return 0
    if a.op == 'dry-run':
        print('DRY RUN: nothing launched. contract %s (PROVISIONAL), FAKE tools, performance UNMEASURED' % CONTRACT)
        print(json.dumps({'plan_sha256': plan['plan_sha256'], **simulate(plan, a.parallel)}, indent=2))
        return 0
    if a.op == 'fake-run':
        if not a.out or a.out.exists():
            print('--out must be a new directory', file=sys.stderr)
            return 2
        res = schedule(plan, FakeWorker(), a.out, a.parallel)
        if res['passed']:
            ordered = {u: sha((a.out / 'units' / u / 'object.bin').read_bytes()) for u in plan['order']}
            FakeLinker()(plan, ordered, a.out / 'link')
        (a.out / 'plan.json').write_text(json.dumps(plan, sort_keys=True))
        why = verify_run(a.root, a.out, FAKE_TOOLS, a.out / 'plan.json', allow_fake=True) if res['passed'] else ['schedule failed']
        print(json.dumps({'schedule': res, 'verify': why or 'verified (FAKE workers; says nothing about a real compiler)'}, indent=2)[:2000])
        return 0 if not why else 1
    why = verify_run(a.root, a.run, FAKE_TOOLS, a.run / 'plan.json', allow_fake=False)
    print('\n'.join(why) if why else 'verified')
    return 1 if why else 0


if __name__ == '__main__':
    raise SystemExit(main())
