#!/usr/bin/env python3
"""Prepare exact-text focused UI/proof inputs; this is not a full proof gate."""
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('proof_slice', ROOT / 'scripts/proof-slice.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
slicer = module.Slicer(ROOT)
seeds = slicer.blocks('test/model_selector_render_test.bend')
seeds += slicer.law_seeds('model_catalog_legacy_passthrough')
output = ROOT / 'build/model-selector-focus'
kept = slicer.write_shard(seeds, output)
# write_shard independently verifies declaration hashes and kept import reachability.
files = sorted({path for path, _, _ in kept})
receipt = {'scope': 'Focused selector UI and one added law only; not the full application/proof gate',
           'declarations': len(kept), 'source_sha256': {
               path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in files}}
(output / 'focus.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('Focused source prepared:', len(kept), 'exact-text declarations in', len(files), 'modules')
