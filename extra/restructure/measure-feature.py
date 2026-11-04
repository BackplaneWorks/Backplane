"""Measure lexical closures on Boat; this is source analysis, never a proof gate."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--identity', required=True, type=Path)
parser.add_argument('--module', required=True)
parser.add_argument('--law', required=True, action='append')
parser.add_argument('--out', required=True, type=Path)
args = parser.parse_args()
if os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') != '1':
    raise SystemExit('Boat required')
group = Path('/sys/fs/cgroup') / next(
    line.split(':', 2)[2].strip().lstrip('/')
    for line in Path('/proc/self/cgroup').read_text().splitlines() if line.startswith('0::'))
cap = (group/'memory.max').read_text().strip()
swap = (group/'memory.swap.max').read_text().strip()
if cap == 'max' or int(cap) > 536870912 or swap != '0':
    raise SystemExit('own <=512 MiB/no-swap scope required')
spec = importlib.util.spec_from_file_location('dependency_map', 'scripts/dependency-map.py')
mapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mapper)
root = Path.cwd()
identity = json.loads(args.identity.read_text())


def compact(report):
    return {key: report[key] for key in ['method', 'source_identity', 'totals']} | {
        'largest_law': report['laws'][0],
        'affected_laws': [law for law in report['laws'] if law['name'] in args.law],
        'module': next((m for m in report['modules'] if m['module'] == args.module), None)}


after = mapper.analyze(root, ['PROOF.bend', 'src/server/server.bend'])
args.out.mkdir(parents=True, exist_ok=True)
(args.out/'after-sources.json').write_text(json.dumps(after['sources'], indent=2)+'\n')
result = {'after': compact(after)}
sources = list(after['sources'])
del after
with tempfile.TemporaryDirectory(prefix='feature-map-') as temp:
    base = Path(temp)
    # A narrowed import boundary can remove modules from the after graph that
    # the restored baseline still imports. Preserve unchanged source modules
    # too, instead of assuming the two closures contain the same files.
    available = set(sources) | {p.relative_to(root).as_posix() for p in (root/'src').rglob('*.bend')}
    for name in sorted(available):
        target = base/name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root/name, target)
    for name, source in identity['base_files'].items():
        (base/name).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root/source, base/name)
    before = mapper.analyze(base, ['PROOF.bend', 'src/server/server.bend'])
    (args.out/'before-sources.json').write_text(json.dumps(before['sources'], indent=2)+'\n')
    result['before'] = compact(before)
result['baseline'] = identity['merge_snapshot']['main']
result['scope'] = 'lexical source analysis only; not proof or full-gate evidence'
result['cgroup'] = {'path': str(group), 'memory_max_bytes': int(cap),
                    'memory_swap_max_bytes': int(swap),
                    'memory_peak_bytes': int((group/'memory.peak').read_text()),
                    'events': (group/'memory.events').read_text()}
(args.out/'METRICS.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(result))
