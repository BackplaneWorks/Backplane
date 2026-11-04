#!/usr/bin/env python3
"""Focused OMP regression and canonical gate attempts, only on capped Boat."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == '1'
group=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
assert 0<int((group/'memory.max').read_text())<=3700000000
assert (group/'memory.swap.max').read_text().strip()=='0'
out=Path('build/omp-launch-checks');out.mkdir(parents=True,exist_ok=True)
results=[]
def run(name,cmd,seconds=100):
    start=time.monotonic()
    with (out/(name+'.log')).open('wb') as log:
        child=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        timedout=False
        try: rc=child.wait(seconds)
        except subprocess.TimeoutExpired:
            timedout=True;os.killpg(child.pid,signal.SIGTERM)
            try:rc=child.wait(5)
            except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);rc=child.wait()
    text=(out/(name+'.log')).read_text(errors='replace')
    results.append(dict(name=name,command=cmd,exit=rc,timedOut=timedout,seconds=round(time.monotonic()-start,2),passed=rc==0 and not any(l.startswith(('FAIL','Error:')) for l in text.splitlines())))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(name,results[-1],text[-7000:],flush=True)
    return results[-1]['passed']
os.environ.update(BEND_NO_TELEMETRY='1',BUN_JSC_maxPerThreadStackUsage='536870912',BUN_JSC_forceRAMSize='4294967296')
run('guide',['bend','guide'])
run('shape',['python3','scripts/laws-shape.py'])
run('emit',['bend','test/omp_launch_test.bend','-o',str(out/'fixture.mjs')])
if (out/'fixture.mjs').exists():run('regression',['bun','test/tools/omp_launch.mjs',str(out/'fixture.mjs')])
# A declaration slice includes the actual changed hub integration without a
# whole-server emit. It is scoped evidence, never called a full proof pass.
import importlib.util
spec=importlib.util.spec_from_file_location('slicer','scripts/proof-slice.py')
sl=importlib.util.module_from_spec(spec);spec.loader.exec_module(sl)
s=sl.Slicer(Path.cwd());seeds=[]
for name in ['OmpRun.go','OmpRun.prepared','Grok.failed']:
    seeds+=s.blocks('src/server/server.bend',name)
s.write_shard(seeds,out/'integration')
run('integration-check',['sh','-c','. scripts/bendok.sh; bend_check "$1"','check',str(out/'integration/PROOF.bend')])
if '--canonical' in sys.argv:
    run('canonical-check',['sh','scripts/check.sh','check'],120)
    # Keep the collector outside the proof's smaller capped cgroup so an
    # OOM cannot erase the qualification result. Never loosen the outer cap.
    run('canonical-proof',['systemd-run','--user','--scope','-q','-p','MemoryMax=3200000000','-p','MemorySwapMax=0','-p','OOMPolicy=continue','--','bend','PROOF.bend'],120)
(out/'kernel.json').write_text(json.dumps({n:(group/n).read_text().strip() for n in ['memory.max','memory.swap.max','memory.peak','memory.events']},indent=2)+'\n')
raise SystemExit(0 if all(x['passed'] for x in results if not x['name'].startswith('canonical-')) else 1)
