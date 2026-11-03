"""Capped, sole-slot synthetic compiler controls. Never builds/runs Backplane."""
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
import time

ROOT = Path(os.environ.get('BP_MODULE_TEST_ROOT', '/home/h/Documents/i2cjak/backplane-bend/.backplane/worktrees/ci-speed-next-1261555'))
COMPILER = Path(os.environ.get('BP_MODULE_COMPILER_ROOT', '/home/h/Documents/i2cjak/bend-memory-1261555'))
OUT = ROOT / 'build/ci-speed/compiler-memory/module-controls'
OUT.mkdir(parents=True, exist_ok=True)
# Preserve installer priority via the outer non-priority --no-wait admission.
# Hold its ledger mutex only for this bounded tiny-fixture batch, so no other
# admitted heavy job can claim a second slot during the declared sole slot.
ledger_root = Path(os.environ.get('XDG_STATE_HOME', str(Path.home()/'.local/state'))) / 'bp-admit'
admission = (ledger_root / 'ledger.lock').open('a+')
try:
    fcntl.flock(admission, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    raise SystemExit(75)
mine = int(os.environ['BP_CI_RESERVATION_PID'])
live = [int(line.split()[0]) for line in (ledger_root/'ledger').read_text().splitlines()
        if line.split() and Path('/proc', line.split()[0]).exists()]
priority = ledger_root/'priority'
if priority.exists() and any(Path('/proc', p).exists() for p in priority.read_text().split() if p.isdigit() and int(p) != mine):
    print('installer priority pending; no fixtures started', flush=True)
    raise SystemExit(75)
if live != [mine]:
    print('sole-slot admission unavailable', flush=True)
    raise SystemExit(75)
group = Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
cg = Path('/sys/fs/cgroup')/group
assert int((cg/'memory.max').read_text()) <= 2 * 1024**3
assert (cg/'memory.swap.max').read_text().strip() == '0'
spec = importlib.util.spec_from_file_location('guard', ROOT/'scripts/heavy-guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
held = guard.inherited_lock(guard.lock_path())
native = None
if held is None:
    native = guard.lock_path().open('a+')
    try:
        fcntl.flock(native, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print('actual bend flock unavailable; no fixtures started', flush=True)
        raise SystemExit(75)
resource.setrlimit(resource.RLIMIT_STACK, (1024**3, resource.getrlimit(resource.RLIMIT_STACK)[1]))
env = {**os.environ, 'BEND_NO_TELEMETRY': '1', 'BUN_CONFIG_NO_ANALYTICS': '1',
       'DO_NOT_TRACK': '1', 'BUN_JSC_forceRAMSize': str(1024**3),
       'BUN_JSC_maxPerThreadStackUsage': str(512*1024**2),
       'BUN_JSC_numberOfGCMarkers': '1', 'BUN_JSC_numberOfDFGCompilerThreads': '1',
       'BUN_JSC_numberOfFTLCompilerThreads': '1'}
BUN = os.environ.get('BP_MODULE_BUN', 'bun')
started = time.monotonic()
rows = []
cases = [
    ('diamond', {'leaf.bend':'import Base\ndef inc(x: U32) -> U32:\n  (x + 1 : U32)\n',
                 'left.bend':'import Base\nimport ./leaf.bend as L\ndef left(x: U32) -> U32:\n  L.inc(x)\n',
                 'right.bend':'import Base\nimport ./leaf.bend as L\ndef right(x: U32) -> U32:\n  L.inc(x)\n',
                 'main.bend':'import Base\nimport ./left.bend as A\nimport ./right.bend as B\ndef main() -> U32:\n  (A.left(1) + B.right(2) : U32)\n'}, 'main.bend', True, True),
    ('template', {'lib.bend':'import Base\ndef inc(x: U32) -> U32:\n  (x + 1 : U32)\ndef twice(~f: U32 -> U32, x: U32) -> U32:\n  f(f(x))\n',
                  'main.bend':'import Base\nimport ./lib.bend as L\ndef main() -> U32:\n  L.twice(~L.inc, 2)\n'}, 'main.bend', True, True),
    ('false-proof', {'main.bend':'import Base\ndef bad() -> {0n == 1n : Nat}:\n  {==}\n'}, 'main.bend', False, False),
    ('import-type-error', {'lib.bend':'import Base\ndef wrong() -> U32:\n  "wrong"\n',
                          'main.bend':'import Base\nimport ./lib.bend as L\ndef main() -> U32:\n  L.wrong()\n'}, 'main.bend', False, False),
    ('cycle', {'a.bend':'import Base\nimport ./main.bend as M\ndef a() -> U32:\n  1\n',
               'main.bend':'import Base\nimport ./a.bend as A\ndef main() -> U32:\n  A.a()\n'}, 'main.bend', False, False),
    ('missing', {'main.bend':'import Base\nimport ./missing.bend as M\ndef main() -> U32:\n  M.main()\n'}, 'main.bend', False, False),
]
cases.append(('datatype', {'lib.bend':'import Base\ntype Wrap is Data:\n  Wrap{value: U32}\ndef get(w: Wrap) -> U32:\n  match w:\n    case Wrap{v}:\n      v\n',
                            'main.bend':'import Base\nimport ./lib.bend as L\ndef main() -> U32:\n  L.get(L.Wrap{42})\n'}, 'main.bend', True, True))
for name, claim in [('law-pass', '0n == 0n'), ('law-fail', '0n == 1n')]:
    cases.append((name, {'LAWS.bend':'import Base\nlaw identity:\n  {'+claim+' : Nat}\n',
                         'PROOF.bend':'import Base\nimport ./LAWS.bend as L\ndef L.identity():\n  {==}\n'},
                  'PROOF.bend', name == 'law-pass', False))
for filename in ['float_roundtrip.bend', 'json.bend', 'heap_queue_deque.bend']:
    cases.append((filename, {'main.bend': (COMPILER/'tests/base'/filename).read_text()},
                  'main.bend', filename == 'float_roundtrip.bend', filename == 'float_roundtrip.bend'))

# Exact unchanged compiler/modes already qualified these six cases in v2.
# Reuse those receipts; the malformed equality controls require fresh checks.
reused_cases = ['diamond', 'template', 'import-type-error', 'cycle', 'missing', 'datatype']
cases = [case for case in cases if case[0] not in reused_cases]
with tempfile.TemporaryDirectory(prefix='bend-modules-') as tmp:
    for name, files, entry, passing, modular in cases:
        folder = Path(tmp)/name
        folder.mkdir()
        for filename, text in files.items():
            (folder/filename).write_text(text)
        hashes = []
        for mode in ['full', 'modules', 'compact']:
            emit = folder/(mode+'.js')
            profile = folder/(mode+'.jsonl')
            args = [str(folder/entry), '-o', str(emit)] if passing and entry != 'PROOF.bend' else [str(folder/entry), '--check-only']
            result = subprocess.run([BUN, str(COMPILER/'bend2/main.ts'), *args],
                env={**env, 'BP_BEND_MODULES': '0' if mode == 'full' else '1',
                     'BP_BEND_COMPACT': '1' if mode == 'compact' else '0',
                     'BP_COMPILER_PROFILE': str(profile)}, text=True, capture_output=True, timeout=15)
            (OUT/(name+'-'+mode+'.txt')).write_text(result.stdout+result.stderr)
            assert (result.returncode == 0) == passing, (name, mode, result.stdout, result.stderr)
            if name in ['false-proof', 'law-fail', 'import-type-error']:
                assert 'Location:' in result.stderr and '- expected :' in result.stderr
                assert "expected : ':'" not in result.stderr and 'observed : Empty' not in result.stderr
                assert 'Location: bad' in result.stderr if name == 'false-proof' else True
                assert 'Location: ' in result.stderr if name == 'law-fail' else True
            marks=[json.loads(line) for line in profile.read_text().splitlines()] if profile.exists() else []
            if passing and modular:
                assert any(x['phase']=='modules:complete' for x in marks) == (mode != 'full'), (name,mode,marks)
            if entry == 'PROOF.bend':
                assert not any(x['phase']=='modules:complete' for x in marks)
            if emit.exists():
                hashes.append(hashlib.sha256(emit.read_bytes()).hexdigest())
            rows.append({'case': name, 'mode': mode, 'exit': result.returncode,
                         'modules_checked': any(x['phase']=='modules:complete' for x in marks),
                         'emit_sha256': hashlib.sha256(emit.read_bytes()).hexdigest() if emit.exists() else None,
                         'seconds': marks[-1]['seconds'] if marks else None})
        assert not hashes or len(set(hashes))==1, (name, hashes)
        print('ok', name, 'all modes agree', flush=True)
actual=hashlib.sha256((COMPILER/'bend2/bend.ts').read_bytes()).hexdigest()
original=os.environ.get('BP_MODULE_BASELINE_CHECKER_SHA256') or hashlib.sha256(subprocess.check_output(['git','show','v2.0.36:bend2/bend.ts'],cwd=COMPILER)).hexdigest()
assert actual == original
receipt={'kind':'tiny synthetic module-check controls only; no whole-hub memory qualification',
         'seconds':time.monotonic()-started,'rows':rows,'checker_sha256':actual,
         'reused_cases':reused_cases, 'reused_manifest_sha256':'d9e8a49619a0ff11c7d4760aa060c447af1de3501d9bef063f9fe960bee323d6',
         'sources_sha256':{n:hashlib.sha256((COMPILER/'bend2'/n).read_bytes()).hexdigest()
                           for n in ['main.ts','comp.ts','modules.ts','profile.ts']},
         'memory_max_bytes':int((cg/'memory.max').read_text()), 'swap_max':0,
         'actual_bend_flock_held':True,'sole_slot_reservation_pid':mine,
         'aggregate_peak_bytes':int((cg/'memory.peak').read_text()),
         'max_child_rss_kib':resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss}
(OUT/'RESULT.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'cases':len(cases),'invocations':len(rows),'seconds':receipt['seconds']}),flush=True)
