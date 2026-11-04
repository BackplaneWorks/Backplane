"""Mocked before/after admission-routing fixture; never executes a child."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import time
import types
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def load(source, name):
    module = types.ModuleType(name)
    module.__file__ = str(ROOT/'scripts/heavy-guard.py')
    exec(compile(source, module.__file__, 'exec'), module.__dict__)
    return module


def measure():
    base = (HERE/'BASE.txt').read_text().strip()
    before = subprocess.check_output(['git', 'show', base+':scripts/heavy-guard.py'], cwd=ROOT)
    after = (ROOT/'scripts/heavy-guard.py').read_bytes()
    command = [str(ROOT/'scripts/build-js.sh'), 'src/server/main.bend', 'dist/service']
    environment = {'BACKPLANE_PREEMITTED_JS': '/fixture/retained.js',
                   'BACKPLANE_PREEMITTED_SHA256': 'a'*64, 'BACKPLANE_PREEMITTED_KEY': 'b'*40}
    cases = [('whole-proof', ['bend', 'PROOF.bend'], {}),
             ('unrelated-bundle', ['bun', 'build', '--compile', 'source.js'], {}),
             ('foreign-entrypoint', ['/other/scripts/build-js.sh', *command[1:]], {}),
             ('partial-retained-env', command, {'BACKPLANE_PREEMITTED_KEY': ''}),
             ('valid-retained-route', command, {})]
    records = {}
    modules = [load(before, 'guard_before'), load(after, 'guard_after')]
    assert modules[0].FLOORS == modules[1].FLOORS
    assert modules[0].HOST_RESERVE == modules[1].HOST_RESERVE
    for label, module, source in zip(('before', 'after'), modules, (before, after)):
        rows = []
        for name, argv, overrides in cases:
            errors = io.StringIO()
            with patch.dict(os.environ, {**environment, **overrides}), \
                 patch.object(module, 'inherited_lock', return_value=999), \
                 patch.object(module, 'admit', return_value=True), \
                 patch.object(module.os, 'set_inheritable'), \
                 patch.object(module.os, 'execvpe') as execute, \
                 patch.object(module, 'offload') as offload, \
                 contextlib.redirect_stderr(errors):
                start = time.perf_counter()
                result = module.main(['compile-js', '--', *argv])
                seconds = time.perf_counter() - start
            rows.append({'case': name, 'guard_return': result,
                         'guard_stderr': errors.getvalue().strip(),
                         'mock_exec_requested': execute.called, 'mock_offload_requested': offload.called,
                         'mocked_routing_seconds': seconds})
        records[label] = {'guard_sha256': hashlib.sha256(source).hexdigest(), 'cases': rows}
    assert all(row['mock_exec_requested'] for row in records['before']['cases'])
    assert all(not row['mock_exec_requested'] and row['guard_return'] == 2
               for row in records['after']['cases'][:-1])
    assert records['after']['cases'][-1]['mock_exec_requested']
    return {'scope': 'mocked admission only; no real lock, host-memory claim, child, proof, Bun or offload execution',
            'base_commit': base, 'records': records, 'actual_heavy_executions': 0,
            'paid_cost_dollars': 0, 'floors_and_host_reserve_unchanged': True,
            'full_build_or_gate_qualification': False}


if __name__ == '__main__':
    print(json.dumps(measure(), sort_keys=True, indent=2))
