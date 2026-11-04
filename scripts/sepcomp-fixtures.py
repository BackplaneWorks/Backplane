#!/usr/bin/env python3
"""Separate-compilation semantic fixtures: what the stock Bend checker does at module boundaries.

Fixture sources are stored as `*.bend.txt` so that check.sh (which type-checks every tracked `*.bend`) and the
content-cache / inputs-key inventories never ingest these deliberately failing modules; `run` materialises a
case into a temporary directory with the `.txt` suffix removed.

`list` needs no compiler. `run` executes `bend MAIN.bend --check-only` for every case and compares the
classification with test/separate-compilation/expected.json. It REFUSES (exit 75) unless ALL of these hold:
  1. the process inherited the actual native bend flock (BACKPLANE_HEAVY_LOCK_FD on /tmp/bp-bend.lock), i.e.
     an admitted executor started it; this script cannot take the lock itself;
  2. its own cgroup has a finite memory.max <= 3,700,000,000 B and memory.swap.max = 0;
  3. heavy-guard admission passes (memory headroom and host reserve, profile proof-shard).
heavy-guard has no profile that execs this script (its worker contract is deliberately narrow), so a run needs
an executor that already holds the guard; there is no fallback. This is not a proof run and qualifies nothing.
"""
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT/'test/separate-compilation'


def guard():
    spec = importlib.util.spec_from_file_location('heavy_guard', ROOT/'scripts/heavy-guard.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def classify(rc, text):
    """pass | unsafe-only | fail, using the repository's own verdict (scripts/bendok.sh)."""
    ok = subprocess.run(['sh', '-c', '. "$1"; bend_verdict "$2" "$3"', 'v', str(ROOT/'scripts/bendok.sh'), str(rc), text]).returncode == 0
    if not ok:
        return 'fail'
    return 'pass' if 'ALL PROOFS CHECK' in text or 'All terms check' in text else 'unsafe-only'


def expected():
    table = json.loads((FIXTURES/'expected.json').read_text())
    cases = sorted(p.name for p in (FIXTURES/'cases').iterdir() if p.is_dir())
    if sorted(table) != cases:
        raise ValueError('expected.json and cases/ differ: %s' % sorted(set(table) ^ set(cases)))
    for name in cases:
        if not (FIXTURES/'cases'/name/'MAIN.bend.txt').is_file():
            raise ValueError('case without MAIN.bend: ' + name)
    return table


def materialise(name, into):
    case = FIXTURES/'cases'/name
    target = Path(into)/name
    for source in case.rglob('*.bend.txt'):
        dest = target/source.relative_to(case).with_suffix('')
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest)
    return target/'MAIN.bend'


def run(bend='bend'):
    g = guard()
    if g.inherited_lock(g.lock_path()) is None:
        print('sepcomp-fixtures: needs the actual inherited bend flock from an admitted executor; refusing', file=sys.stderr)
        return 75
    if not g.verify_shard_cgroup():
        print('sepcomp-fixtures: needs its own cgroup with memory.max <= 3700000000 and memory.swap.max = 0; refusing', file=sys.stderr)
        return 75
    if not g.admit('proof-shard'):
        print('sepcomp-fixtures: heavy-guard admission refused; no fallback', file=sys.stderr)
        return 75
    table, results = expected(), {}
    with tempfile.TemporaryDirectory() as work:
        for name, want in table.items():
            main = materialise(name, work)
            p = subprocess.run([bend, str(main), '--check-only'], capture_output=True, text=True, timeout=300)
            got = classify(p.returncode, p.stdout + p.stderr)
            results[name] = {'expected': want['verdict'], 'got': got, 'rc': p.returncode}
    cg = g.own_cgroup()
    receipt = {'cases': results, 'memory_max': (cg/'memory.max').read_text().strip(), 'swap_max': (cg/'memory.swap.max').read_text().strip(),
               'peak': (cg/'memory.peak').read_text().strip() if (cg/'memory.peak').exists() else None}
    print(json.dumps(receipt, indent=1))
    return 0 if all(r['expected'] == r['got'] for r in results.values()) else 1


def main(argv):
    if argv[:1] == ['list']:
        for name, want in expected().items():
            print('%-44s %-12s %s' % (name, want['verdict'], want['why']))
        return 0
    if argv[:1] == ['run']:
        return run(*argv[1:2])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
