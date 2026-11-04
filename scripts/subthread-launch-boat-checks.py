#!/usr/bin/env python3
"""Capped Boat evidence; declaration slices are scoped, never a full proof pass."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time
import sys
ROOT = Path.cwd()
OUT = ROOT/'build/subthread-launch-checks'
OUT.mkdir(parents=True, exist_ok=True)
assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == '1'
CG = Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
def kernel():
    return {n:(CG/n).read_text().strip() for n in ['memory.max','memory.swap.max','memory.peak','memory.events']}
assert 0 < int(kernel()['memory.max']) <= 3700000000 and kernel()['memory.swap.max'] == '0'
os.environ.update(BEND_NO_TELEMETRY='1', BUN_JSC_maxPerThreadStackUsage='536870912', BUN_JSC_forceRAMSize='4294967296')
os.environ['PATH'] = str(Path.home()/'.bend/bin') + ':' + os.environ['PATH']
results = []
def run(name, cmd, bend=False, seconds=90):
    start = time.monotonic(); before = kernel(); log = OUT/(name+'.log')
    with log.open('wb') as f:
        child = subprocess.Popen(cmd, stdout=f, stderr=subprocess.STDOUT, start_new_session=True)
        timeout = False
        try: code = child.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            timeout = True; os.killpg(child.pid, signal.SIGTERM)
            try: code = child.wait(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(child.pid, signal.SIGKILL); code = child.wait()
    text = log.read_text(errors='replace')
    good = code == 0 and not any(l.startswith(('FAIL', 'Error:')) for l in text.splitlines())
    if bend:
        good = subprocess.run(['sh','-c','. scripts/bendok.sh; bend_verdict "$1" "$(cat "$2")"','v',str(code),str(log)]).returncode == 0
    item = dict(name=name, command=cmd, passed=good and not timeout, exit=code, timed_out=timeout, seconds=round(time.monotonic()-start,3), before=before, after=kernel(), peak_child_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
    results.append(item); (OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(item), flush=True); print(text[-6000:], flush=True)
    return good
run('guide', ['bend','guide'])
run('shape', ['python3','scripts/laws-shape.py'])
run('repair-fixture', ['python3','-m','unittest','discover','-s','test/tools','-p','coordinator_parent_repair_test.py'])
spec = importlib.util.spec_from_file_location('slicer', ROOT/'scripts/proof-slice.py')
sl = importlib.util.module_from_spec(spec); spec.loader.exec_module(sl)
roots = {
    'laws': [('LAWS.bend','mcp_subthread_seed_batch'), ('PROOF.bend','Laws.mcp_subthread_seed_batch'), ('LAWS.bend', 'mcp_subthread_link'), ('PROOF.bend', 'Laws.mcp_subthread_link'), ('LAWS.bend', 'mcp_subthread_parent'), ('PROOF.bend', 'Laws.mcp_subthread_parent'), ('LAWS.bend', 'mcp_subthread_manual_move_wins'), ('PROOF.bend', 'Laws.mcp_subthread_manual_move_wins'), ('LAWS.bend', 'sdb_up_parents_demo'), ('PROOF.bend', 'Laws.sdb_up_parents_demo'), ('LAWS.bend','subs_count_ix_eq'), ('PROOF.bend','Laws.subs_count_ix_eq')],
    'coordinator': [('test/subthread_launch_test.bend','main')],
    'reports': [('test/launch_test.bend','main')],
    'hierarchy': [('test/subthreads_test.bend','main')],
    'remote': [('test/fmcp_test.bend','main')],
    'autosettle': [('test/autosettle_test.bend','main')],
    'instructions': [('src/core/mcp.bend','Mcp.tools'), ('src/core/prompt.bend','Prompt.orchestration')],
}
for name, rs in roots.items():
    s = sl.Slicer(ROOT); seeds = []
    for rel, symbol in rs:
        blocks = s.blocks(rel, symbol); assert blocks, (rel,symbol); seeds += blocks
    dest = OUT/name; kept = s.write_shard(seeds,dest)
    run(name+'-check',['bend',str(dest/'PROOF.bend'),'--check-only'],bend=True)
    tests = [rel for rel, symbol in rs if rel.startswith('test/') and symbol == 'main']
    for rel in tests: run(name+'-test',['bend',str(dest/'t'/rel)])
# Canonical proof is attempted once. Retain an honest incomplete/OOM verdict.
if '--canonical' in sys.argv:
    run('canonical-proof', ['bend','PROOF.bend'], bend=True, seconds=120)
(OUT/'source-sha256.json').write_text(json.dumps({str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [Path('LAWS.bend'),Path('PROOF.bend'),*Path('src/core').glob('*.bend'),Path('test/subthread_launch_test.bend')]},indent=2)+'\n')
raise SystemExit(0 if all(r['passed'] for r in results if r['name'] != 'canonical-proof') else 1)
