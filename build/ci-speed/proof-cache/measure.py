#!/usr/bin/env python3
"""Measure key retention on a Git export; never execute or certify a proof."""
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', required=True, help='Git revision of old cache helper')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    old = subprocess.check_output(['git', 'show', args.before+':scripts/content-cache.py'], cwd=repo)
    source_revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
    # Canonical proofs also import test/ modules. Export the entire tracked
    # tree so root discovery can fail closed on any missing dependency.
    archive = subprocess.check_output(['git', 'archive', 'HEAD'], cwd=repo)
    with tempfile.TemporaryDirectory(prefix='bp-proof-key-measure-') as temp:
        temp = Path(temp)
        root = temp/'source'
        root.mkdir()
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            tar.extractall(root, filter='data')
        old_path = temp/'before.py'
        old_path.write_bytes(old)
        variants = {'before': load(old_path, 'before_cache'),
                    'after': load(repo/'scripts/content-cache.py', 'after_cache')}
        variants['before'].ROOT = repo
        ps = variants['after']._module('proof-shards.py')
        folder = root/'build/key-measure/shards'
        def generate():
            manifest, files = ps.plan(root, folder, 128)
            for name, body in files.items():
                p = folder/name
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_bytes(body)
            (folder/'manifest.json').write_bytes(ps.encoded(manifest))
            return manifest
        manifest = generate()
        before, rows = {}, {}
        for name, mod in variants.items():
            start = time.perf_counter()
            units, _ = mod.shard_units(root, folder)
            before[name] = {key for key, _ in units.values()}
            rows[name] = {'initial_key_plan_seconds': time.perf_counter()-start}
        target = manifest['laws'][0]
        path = root/'PROOF.bend'
        original = path.read_text()
        blocks = ps.parse(original, 'PROOF.bend')[1]
        block = next(b['text'] for b in blocks if b['name'] == 'Laws.'+target)
        changed = block.rstrip('\n')+'\n  # cache-key mutation measurement\n\n'
        assert block != changed and original.count(block) == 1
        path.write_text(original.replace(block, changed, 1))
        generate()
        for name, mod in variants.items():
            start = time.perf_counter()
            units, _ = mod.shard_units(root, folder)
            keys = [key for key, _ in units.values()]
            rows[name].update(changed_key_plan_seconds=time.perf_counter()-start,
                              retained_keys=sum(k in before[name] for k in keys),
                              invalidated_keys=sum(k not in before[name] for k in keys),
                              total_shards=len(keys))
        report = {'kind': 'offline canonical key retention; NO proof execution or pass recording',
                  'source_revision': source_revision, 'before_revision': args.before,
                  'before_helper_sha256': hashlib.sha256(old).hexdigest(),
                  'after_helper_sha256': hashlib.sha256((repo/'scripts/content-cache.py').read_bytes()).hexdigest(),
                  'toolchain': manifest['toolchain'], 'canonical_laws': len(manifest['laws']),
                  'mutation': 'one comment line in def Laws.'+target,
                  'results': rows, 'paid_cost_dollars': 0,
                  'limitations': 'n=1, sequential same host; timings are key planning, not CI or proof time; local cache only'}
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, indent=2, sort_keys=True)+'\n')
        print(json.dumps({k: v for k, v in report.items() if k != 'toolchain'}, sort_keys=True))


if __name__ == '__main__':
    main()
