"""Declaration closures for the containment checks (descriptor-bound opens,
staged converter inputs, published renders). Scoped evidence, not a
substitute for the full checks."""
from pathlib import Path
import importlib.util
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('slice', ROOT/'scripts/proof-slice.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
slicer = module.Slicer(ROOT)
scopes = {
    'stream': [('test/far_stream_test.bend', 'main')],
    'zip': [('src/server/zip.bend', 'Zip.publish'), ('src/server/zip.bend', 'Zip.stage'), ('src/server/zip.bend', 'Zip.scratch')],
    'farview': [('src/server/farview.bend', 'Fvs.get')],
    'render': [('src/app/render.bend', 'Render.main')],
    'plot': [('src/server/plot.bend', 'Pv.read.at')],
    'mech': [('src/server/server.bend', 'Mech.render')],
    'conn': [('src/server/server.bend', 'Conn.bot.mark')],
    'view': [('src/server/server.bend', 'Hub.bot.view'), ('src/server/server.bend', 'Hub.far.view.k')],
}
for name, roots in scopes.items():
    seeds = [b for path, symbol in roots for b in slicer.blocks(path, symbol, 'def')]
    assert len(seeds) == len(roots), name
    kept = slicer.write_shard(seeds, ROOT/'build/contain'/name)
    print(name, len(kept), 'byte-verified canonical declarations')
