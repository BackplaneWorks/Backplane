"""Real cache-branch fixtures and saved-artifact byte workload; fake tools only."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
import types
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def load_fixture():
    path = ROOT/'test/tools/emit_cache_test.py'
    spec = importlib.util.spec_from_file_location('cache_fixture', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def measure():
    fixture = load_fixture()
    base = (HERE/'BASE.txt').read_text().strip()
    old_build = subprocess.check_output(['git', 'show', base+':scripts/build-js.sh'], cwd=ROOT)
    old_local = subprocess.check_output(['git', 'show', base+':scripts/local.py'], cwd=ROOT)
    # This exact public output is already preserved by PR879. It is NEVER run.
    with tarfile.open(ROOT/'build/ci-speed/graph2-review/EVIDENCE.tar.gz') as archive:
        payload = archive.extractfile('mjs/focused.mjs').read()
    payload_sha = hashlib.sha256(payload).hexdigest()
    assert payload_sha == '96706cca9467256056bc2fee70b5905ac9547d624eef6fb086452a52200e30a8'
    observations = {}
    for label, source in [('before', old_build), ('after', (ROOT/'scripts/build-js.sh').read_bytes())]:
        rows = []
        for case in ('valid-cache', 'same-size-corruption', 'legacy-cache', 'saved-output-byte-workload'):
            with tempfile.TemporaryDirectory(prefix='bp-emit-measure-') as directory:
                f = fixture.BuildCacheHarness(directory, build_source=source)
                raw = payload if case == 'saved-output-byte-workload' else fixture.RAW
                prepared = f.root/'input.js'; prepared.write_bytes(raw)
                fixture.cache.publish(f.store, fixture.KEY, prepared)
                data, receipt = fixture.cache.paths(f.store, fixture.KEY)
                if label == 'before' or case == 'legacy-cache':
                    receipt.write_text(str(len(raw)))
                if case == 'same-size-corruption':
                    data.write_bytes(fixture.CHANGED)
                started = time.monotonic()
                result = f.run()
                seconds = time.monotonic()-started
                copied = f.root/'build/js/fixture/all.js'
                expect_miss = label == 'after' and case in ('same-size-corruption', 'legacy-cache')
                assert result.returncode == (98 if expect_miss else 97), result.stderr
                assert (f.root/'bend.marker').exists() == expect_miss
                assert (f.root/'bun.marker').exists() != expect_miss
                rows.append({'case': case, 'bytes': len(raw), 'fixture_seconds': seconds,
                    'fake_runtime_exit': result.returncode, 'fake_emission_requested': expect_miss,
                    'cached_bytes_copied': copied.exists(),
                    'copied_sha256': hashlib.sha256(copied.read_bytes()).hexdigest() if copied.exists() else None})
        observations[label] = rows
    # The local warm-build decision must use the same contract as build-js.
    before_local = types.ModuleType('local_before')
    before_local.__file__ = str(ROOT/'scripts/local.py')
    exec(compile(old_local, before_local.__file__, 'exec'), before_local.__dict__)
    warm = {}
    for label, module in [('before', before_local), ('after', fixture.local)]:
        with tempfile.TemporaryDirectory(prefix='bp-emit-warm-') as directory:
            source = Path(directory)/'input.js'; source.write_bytes(fixture.RAW)
            store = Path(directory)/'cache'
            fixture.cache.publish(store, fixture.KEY, source)
            data, receipt = fixture.cache.paths(store, fixture.KEY)
            if label == 'before': receipt.write_text(str(len(fixture.RAW)))
            with patch.object(module.subprocess, 'check_output', return_value=fixture.KEY), \
                 patch.object(module, 'phone_bridge_cached', return_value=True):
                valid = module.warm_build({'BACKPLANE_EMIT_CACHE': str(store)})
                data.write_bytes(fixture.CHANGED)
                corrupted = module.warm_build({'BACKPLANE_EMIT_CACHE': str(store)})
            assert valid and corrupted == (label == 'before')
            warm[label] = {'valid_cache_warm': valid, 'same_size_corruption_warm': corrupted}
    return {'scope': 'real entrypoint with fake key/guard/runtimes; stops before real emission/compile',
            'base_commit': base, 'before_build_sha256': hashlib.sha256(old_build).hexdigest(),
            'after_build_sha256': hashlib.sha256((ROOT/'scripts/build-js.sh').read_bytes()).hexdigest(),
            'cache_helper_sha256': hashlib.sha256((ROOT/'scripts/emit-cache.py').read_bytes()).hexdigest(),
            'observations': observations, 'local_warm_decision': warm,
            'saved_output_workload': {'bytes': len(payload), 'sha256': payload_sha,
                'source': 'previous focused job j20261104001107-414164, public source output, not executed'},
            'actual_heavy_builds': 0, 'paid_cost_dollars': 0,
            'source_key_is_synthetic_fixture_only': True,
            'full_build_or_proof_or_release_qualification': False}


if __name__ == '__main__':
    print(json.dumps(measure(), sort_keys=True, indent=2))
