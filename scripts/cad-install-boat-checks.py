#!/usr/bin/env python3
"""Boat-only diagnostics. OOMPolicy=continue keeps this collector alive.
Whole imports/proofs keep their honest verdicts; slices are focused checks only.
"""
import importlib.util
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time
import sys
ROOT=Path.cwd();OUT=ROOT/'build/cad-install-test';OUT.mkdir(parents=True,exist_ok=True)
assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1'
CG=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
def kernel():
    return {n:(CG/n).read_text().strip() for n in ['memory.max','memory.swap.max','memory.peak','memory.events']}
before=kernel();assert 0<int(before['memory.max'])<=3_700_000_000 and before['memory.swap.max']=='0'
print('kernel caps: '+json.dumps(before),flush=True)
os.environ.update(BEND_NO_TELEMETRY='1',BUN_JSC_maxPerThreadStackUsage='536870912',BUN_JSC_forceRAMSize='4294967296')
results=[]
def run(name,cmd,mode='exit',seconds=120):
    start=time.monotonic();kb=kernel();log=OUT/(name+'.log');timedout=False
    with log.open('wb') as f:
        p=subprocess.Popen(cmd,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
        try:code=p.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            timedout=True;os.killpg(p.pid,signal.SIGTERM)
            try:code=p.wait(timeout=5)
            except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);code=p.wait(timeout=5)
    text=log.read_text(errors='replace')
    if mode=='bend':
        verdict=subprocess.run(['sh','-c','. scripts/bendok.sh; bend_verdict "$1" "$(cat "$2")"','v',str(code),str(log)]).returncode==0
    else:verdict=code==0 and not any(l.startswith(('FAIL','Error:')) for l in text.splitlines())
    r={'name':name,'command':cmd,'exit':code,'passed':verdict,'timed_out':timedout,'seconds':round(time.monotonic()-start,3),'before':kb,'after':kernel(),'peak_child_rss_kib':resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss}
    results.append(r);(OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(name+': '+json.dumps(r),flush=True);print(text[-5000:],flush=True)
    return verdict
run('fixtures',['node','test/tools/cad_install_test.mjs'])
for pattern in ['cad_install_handler_test.py','freecad_install_test.py','kicad_install_test.py']:
    run(pattern,['python3','-m','unittest','discover','-s','test/tools','-p',pattern])
run('shape',['python3','scripts/laws-shape.py'])
for f in (['src/core/cad-install.bend','src/server/cad-install.bend'] if '--focused' in sys.argv else ['src/core/cad-install.bend','src/server/cad-install.bend','src/core/client.bend','src/web/view.bend','src/app/layout.bend','src/mobile/screen.bend']):
    run(f.replace('/','_'),['bend',f,'--check-only'],'bend')
for f in ['test/cad_install_test.bend','test/kicad_install_test.bend']:
    run(f.replace('/','_'),['bend',f])
# Byte-identical feature roots, including the canonical new laws/proofs.
spec=importlib.util.spec_from_file_location('slicer',ROOT/'scripts/proof-slice.py')
sl=importlib.util.module_from_spec(spec);spec.loader.exec_module(sl)
cfg_helpers=[b['name'] for b in sl.Slicer(ROOT).blocks('src/server/server.bend') if b['kind']=='def' and b['name'].startswith('Cfg.') and 'Cfg{' in b['text']]
slices={
  'laws': [('LAWS.bend','mail_log_count_set'),('PROOF.bend','Laws.mail_log_count_set'),('LAWS.bend','mail_log_count_committed'),('PROOF.bend','Laws.mail_log_count_committed'),('LAWS.bend','cad_install_visible_action'),('PROOF.bend','Laws.cad_install_visible_action'),('LAWS.bend','cad_install_monotonic'),('PROOF.bend','Laws.cad_install_monotonic')],
  'cfg-shape': [('src/server/server.bend',n) for n in cfg_helpers if n not in ['Cfg.thin.committed','Cfg.thin.stored']],
  'server': [('src/server/server.bend',n) for n in ['CadJob.ask','Cfg.with_cad','Cfg.cad','Cfg.with_freecad','Hub.info','Hub.cad.request','Cfg.cad.apply']],
  'ui': [('test/cad_install_ui_test.bend','main')],
}
for name,roots in slices.items():
    s=sl.Slicer(ROOT);seeds=[]
    for rel,n in roots:seeds+=s.blocks(rel,n)
    out=OUT/('slice-'+name);kept=s.write_shard(seeds,out)
    run('slice-'+name,['bend',str(out/'PROOF.bend'),'--check-only'],'bend')
    if name=='ui':run('ui-assertions',['bend',str(out/'t/test/cad_install_ui_test.bend')])
# One canonical proof attempt, with kernel evidence; never call OOM a pass.
if '--focused' not in sys.argv:
    run('canonical-proof',['bend','PROOF.bend'],'bend')
    run('whole-server',['bend','src/server/server.bend','--check-only'],'bend')
whole={'src_core_client.bend','src_web_view.bend','src_app_layout.bend','src_mobile_screen.bend','canonical-proof','whole-server'}
focused=all(r['passed'] for r in results if r['name'] not in whole)
(OUT/'summary.json').write_text(json.dumps({'focused_passed':focused,'whole_results':[r for r in results if r['name'] in whole]},indent=2)+'\n')
print('diagnostics complete; results.json owns each individual verdict',flush=True)
sys.exit(0 if focused else 1)
