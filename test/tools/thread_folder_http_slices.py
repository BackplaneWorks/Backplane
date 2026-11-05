"""A disposable hub using exact production request planning and sink closures.

Both snapshots retain production declarations/effects byte for byte. The
fixture supplies a temporary model and network adapter, never a folder policy.
"""
from pathlib import Path
import importlib.util
import json
import os
import re
import tarfile
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('slice', ROOT/'scripts/proof-slice.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
template = (ROOT/'test/tools/thread_folder_fixture.bend.in').read_text()
aliases = dict((alias, path) for path, alias in re.findall(r'^import ./t/(.*?) as (\w+)$', template, re.M))
modes = ['before', 'fixed'] if os.environ.get('THREAD_FOLDER_BEFORE') == '1' else ['fixed']
for mode in modes:
    folder = ROOT/'build/thread-folder'/mode
    source = ROOT
    if mode == 'before':
        source = folder/'source'; source.mkdir(parents=True, exist_ok=True)
        archive = ROOT/'.ci-packet/thread-folder-before.tar.gz'
        assert archive.is_file(), 'Prepare baseline: git archive --format=tar.gz fc31e97f9772bc5a15b78262cfc05d45cdd32ebf src > .ci-packet/thread-folder-before.tar.gz'
        with tarfile.open(archive) as tar:
            assert tar.pax_headers.get('comment') == 'fc31e97f9772bc5a15b78262cfc05d45cdd32ebf', 'wrong baseline revision'
            tar.extractall(source, filter='data')
    slicer = module.Slicer(source)
    seeds = []
    for token in module.LEX.finditer(template):
        word = token[1]
        if not word or '.' not in word: continue
        alias, name = word.split('.', 1)
        if alias in aliases:
            found = slicer.module(aliases[alias]).find(name)
            assert found, (mode, alias, name)
            seeds.extend(found)
    # host.js registers every public effect when loaded, so retain signatures.
    for name in set(re.findall(r'CID\(([^)]+)\)', (source/'src/server/effects/host.js').read_text())):
        seeds.extend(slicer.blocks('src/server/host.bend', name, 'def'))
    kept = slicer.write_shard(seeds, folder)
    (folder/'http.bend').write_text(template)
    (folder/'scope.json').write_text(json.dumps(sorted(kept)))
    print(mode, len(kept), 'byte-verified production declarations')
