"""Declaration closures for the route-boundary checks (review.bom versions,
kicad.watch folders). Scoped evidence, not a substitute for the full checks."""
from pathlib import Path
import importlib.util
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('slice', ROOT/'scripts/proof-slice.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
slicer = module.Slicer(ROOT)
scopes = {
    'unit': [('test/route_boundary_test.bend', 'main')],
    'git': [('src/server/git.bend', 'Git.show')],
    'review': [('src/server/review.bend', 'RvJob.bom')],
    'regression-rview': [('test/rview_test.bend', 'main')],
    'regression-thread_folder': [('test/thread_folder_test.bend', 'main')],
}
for name, roots in scopes.items():
    seeds = [b for path, symbol in roots for b in slicer.blocks(path, symbol, 'def')]
    assert len(seeds) == len(roots), name
    kept = slicer.write_shard(seeds, ROOT/'build/route-boundary'/name)
    print(name, len(kept), 'byte-verified canonical declarations')
