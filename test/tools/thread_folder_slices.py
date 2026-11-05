"""Exact declaration closures for the shared folder boundary and its proof.

This is scoped evidence, not a substitute for the full project checks/build.
"""
from pathlib import Path
import importlib.util
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('slice', ROOT/'scripts/proof-slice.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
slicer = module.Slicer(ROOT)
scopes = {
    'unit': [('test/thread_folder_test.bend', 'main')],
    'proof': [('PROOF.bend', 'Laws.thread_folder_refuses_job')],
}
for name in ['folder', 'rview', 'fallback', 'scratch', 'mailrun', 'files', 'files_state']:
    scopes['regression-' + name] = [('test/' + name + '_test.bend', 'main')]
for name, roots in scopes.items():
    seeds = [b for path, symbol in roots for b in slicer.blocks(path, symbol, 'def')]
    assert len(seeds) == len(roots)
    kept = slicer.write_shard(seeds, ROOT/'build/thread-folder'/name)
    print(name, len(kept), 'byte-verified canonical declarations')
