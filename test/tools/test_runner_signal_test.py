#!/usr/bin/env python3
"""Exercise the real unit scheduler using tiny fake Bend children only."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

runner = Path('scripts/test.sh').read_text()
sharder = Path('scripts/test-shard.py').read_text()
with tempfile.TemporaryDirectory(prefix='bp-unit-signal-') as tmp:
    root = Path(tmp)
    (root/'scripts').mkdir(); (root/'test').mkdir(); (root/'bin').mkdir()
    script = root/'scripts/test.sh'; script.write_text(runner); script.chmod(0o755)
    verified = root/'scripts/verified.sh'
    verified.write_text('#!/bin/sh\nif [ "$1" = key ]; then echo fixture; else echo marked > "$CONTROL_STATE/verified"; fi\n')
    verified.chmod(0o755)
    (root/'scripts/test-shard.py').write_text(sharder)
    (root/'scripts/test-timings.txt').write_text('4 test/a_test.bend\n2 test/b_test.bend\n1 test/c_test.bend\n')
    for name in ['a', 'b', 'c']:
        (root/f'test/{name}_test.bend').write_text('# synthetic input; never compiled\n')
    bend = root/'bin/bend'
    bend.write_text('''#!/usr/bin/env python3
import json, os, pathlib, signal, sys, time
state = pathlib.Path(os.environ['CONTROL_STATE']); name = pathlib.Path(sys.argv[1]).stem
if os.getenv('CONTROL_BLOCK'):
    def stop(*_):
        # The owned process must be waited for before evidence is cleaned.
        exists = any(pathlib.Path.cwd().glob('.bend-test.*/[01].out'))
        time.sleep(.15)
        (state/(name+'.stopped')).write_text(json.dumps({'evidence_exists':exists}))
        print('ok partial output after termination', flush=True)
        raise SystemExit(143)
    signal.signal(signal.SIGTERM, stop)
    (state/(name+'.pid')).write_text(str(os.getpid()))
    print('ok partial output before termination', flush=True)
    while True: time.sleep(.05)
mode = os.getenv('CONTROL_VERDICT', 'pass')
if mode == 'fail': print('FAIL synthetic assertion')
elif mode != 'silent': print('ok synthetic assertion')
raise SystemExit(7 if mode == 'exit' else 0)
'''); bend.chmod(0o755)
    env = dict(os.environ, PATH=str(root/'bin')+os.pathsep+os.environ['PATH'], TEST_JOBS='2',
               TEST_SHARD='1', TEST_SHARD_COUNT='1')
    def settings(name, **extra):
        state = root/name; state.mkdir()
        return state, dict(env, CONTROL_STATE=str(state), **extra)
    state, e = settings('normal')
    r = subprocess.run([str(script)], cwd=root, env=e, capture_output=True, text=True, timeout=10)
    assert r.returncode == 0 and r.stdout.splitlines() == [f'test/{n}_test.bend: ok' for n in ['a','b','c']]
    assert (state/'verified').is_file() and not list(root.glob('.bend-test.*'))
    print('ok normal verdict, ordered replay, verification marker and successful cleanup are preserved')
    seen=[]
    for shard in ['1','2']:
        state,e=settings('shard'+shard,TEST_SHARD=shard,TEST_SHARD_COUNT='2')
        r=subprocess.run([str(script)],cwd=root,env=e,capture_output=True,text=True,timeout=10)
        assert r.returncode==0 and not (state/'verified').exists()
        seen += [line.split(':')[0] for line in r.stdout.splitlines()]
    assert sorted(seen)==[f'test/{n}_test.bend' for n in ['a','b','c']]
    print('ok duration-based shards still execute every selected test exactly once without a full-suite marker')
    for mode in ['fail', 'silent', 'exit']:
        state, e = settings(mode, CONTROL_VERDICT=mode)
        r = subprocess.run([str(script)], cwd=root, env=e, capture_output=True, text=True, timeout=10)
        assert r.returncode == 1 and not (state/'verified').exists()
        assert not list(root.glob('.bend-test.*'))
    print('ok assertion failure, missing assertions and nonzero child exits still fail normally')
    for sig, code in [(signal.SIGTERM,143),(signal.SIGHUP,129),(signal.SIGINT,130)]:
        state, e = settings(sig.name, CONTROL_BLOCK='1')
        unrelated = subprocess.Popen(['python3','-c','import time; time.sleep(30)'])
        p = subprocess.Popen([str(script)], cwd=root, env=e, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        owned=[]
        try:
            deadline=time.monotonic()+5
            while len(list(state.glob('*.pid'))) < 2 and time.monotonic() < deadline:
                assert p.poll() is None
                time.sleep(.02)
            owned=[int(path.read_text()) for path in state.glob('*.pid')]
            assert len(owned)==2, 'TEST_JOBS=2 did not start two owned children'
            p.send_signal(sig)
            output=p.communicate(timeout=5)[0]
            assert p.returncode==code, (sig,p.returncode,output)
            assert 'incomplete evidence retained' in output and 'Directory nonexistent' not in output
            assert len(list(state.glob('*.pid')))==2, 'scheduler launched a new test after interruption'
            assert len(list(state.glob('*.stopped')))==2, 'parent returned before owned children stopped'
            assert all(json.loads(path.read_text())['evidence_exists'] for path in state.glob('*.stopped'))
            assert not (state/'verified').exists()
            for pid in owned:
                try: os.kill(pid,0)
                except ProcessLookupError: pass
                else: raise AssertionError('owned child was not reaped: '+str(pid))
            assert unrelated.poll() is None, 'unrelated process was signaled'
            work=list(root.glob('.bend-test.*')); assert len(work)==1
            assert all('partial output after termination' in path.read_text() for path in work[0].glob('*.out'))
            shutil.rmtree(work[0])
            print(f'ok {sig.name} retains exit{code}/partial evidence, stops scheduling and forwards/waits only owned children')
        finally:
            if p.poll() is None: p.kill(); p.wait()
            for pid in owned:
                try: os.kill(pid,signal.SIGKILL)
                except ProcessLookupError: pass
            unrelated.terminate(); unrelated.wait()
