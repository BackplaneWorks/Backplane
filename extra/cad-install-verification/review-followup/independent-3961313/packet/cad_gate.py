"""Runtime-only Boat gate. Bind an exact source+staged manifest before use."""
import argparse, hashlib, importlib.util, json, os, platform, shutil, subprocess, sys, time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--inputs',required=True);p.add_argument('--sha256',required=True);p.add_argument('--worker',action='store_true');a=p.parse_args()
sha=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(a.inputs)==a.sha256
m=json.loads(Path(a.inputs).read_text());out=Path('build/cad-independent-v2');out.mkdir(parents=True,exist_ok=True)
def save(name,value):(out/name).write_text(json.dumps(value,indent=2)+'\n')
def git(*args):return subprocess.check_output(['git',*args],text=True).strip()
def admit():
    assert git('show','-s','--format=%s','HEAD')==m['head']
    tracked=set(subprocess.check_output(['git','ls-files','-z']).decode().rstrip('\0').split('\0'))
    assert tracked==set(m['files'])|{a.inputs}
    for name,h in m['files'].items():assert sha(name)==h,name

def kernel():
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1'
    g=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::')[1].strip().lstrip('/')
    return {n:(g/n).read_text().strip() for n in ['memory.max','memory.peak','memory.swap.max','memory.swap.current','memory.swap.peak','memory.events']}
def capped(cap):
    k=kernel();assert 0<int(k['memory.max'])<=cap
    for name in ['memory.swap.max','memory.swap.current','memory.swap.peak']:assert int(k[name])==0
    events=dict((key,int(value))for key,value in (line.split()for line in k['memory.events'].splitlines()))
    for key in ['oom','oom_kill','oom_group_kill']:assert events[key]==0
    return k
admit()
if not a.worker:
    capped(536870912)
    save('SOURCE.json',dict(source_head=m['head'],source_tree=m['tree'],snapshot_oid=git('rev-parse','HEAD'),snapshot_tree=git('rev-parse','HEAD^{tree}'),manifest_sha256=a.sha256,exact_staged_set_and_hashes=True))
    unit='bp-cad-component-'+str(os.getpid())
    cmd=['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000','-p','MemorySwapMax=0','-p','OOMPolicy=continue','--','sh','scripts/heavy-boat.sh',sys.executable,__file__,'--inputs',a.inputs,'--sha256',a.sha256,'--worker']
    with (out/'worker.log').open('w')as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,timeout=1100)
    journal=subprocess.run(['journalctl','--user','-u',unit+'.scope','--no-pager','-n','20'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    (out/'journal.txt').write_text(journal.stdout)
    save('SUMMARY.json',dict(state='passed'if r.returncode==0 else'failed',exit=r.returncode,head=m['head'],controller=capped(536870912),manifest_sha256=a.sha256,worker_log_sha256=sha(out/'worker.log'),command=cmd))
    assert r.returncode==0,(out/'worker.log').read_text()[-5000:]
    admit();print('CAD_INDEPENDENT_GATE_PASS head='+m['head']);sys.exit(0)
started=time.monotonic();records=[]
try:
    admission=capped(3700000000);tools={}
    for name,expected in [('node','v24.13.1'),('bun','1.3.10')]:
        exe=Path(shutil.which(name)).resolve();version=subprocess.check_output([str(exe),'--version'],text=True).strip();assert version==expected
        tools[name]=dict(path=str(exe),sha256=sha(exe),version=version)
    save('TOOLS.json',dict(tools=tools,python=sys.version,hostname=platform.node(),cpu=Path('/proc/cpuinfo').read_text().split('model name')[1].split('\n')[0].lstrip('\t :'),concurrent_load=m['concurrent_load']))
    env=dict(os.environ,CAD_COMPONENT_HEAD=m['head'],DISK_SYNC_TRANSFER_HARNESS='test/perf/hub-latency/effect_transfer.ts')
    cmd=['bun',m['bench'],'--out',str(out/'RESULTS.json')]
    # The benchmark requires --out as argv[2], after Bun's script argv[1].
    with(out/'component.log').open('w')as log:r=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=1000)
    text=(out/'component.log').read_text();records.append(dict(command=cmd,exit=r.returncode,seconds=time.monotonic()-started,log_sha256=sha(out/'component.log')))
    assert r.returncode==0,text[-5000:]
    sentinel='CAD_COMPONENT_PASS tools=2 rounds=32+32 phases=3 bytes=checked children=closed'
    assert text.splitlines().count(sentinel)==1
    assert not any(line.startswith(('CAD_COMPONENT_FAIL','FAIL','not ok'))for line in text.splitlines())
    report=json.loads((out/'RESULTS.json').read_text());assert report['state']=='passed'and report['head']==m['head']
    assert report['identity']['effectSha256']==m['files']['src/server/effects/cad-install.js']
    assert len(report['samples'])==128 and len(report['warmups'])==8
    for tool in ['kicad','freecad']:
        for variant in ['idle','install']:
            for phase in ['download','unpack','cleanup']:
                for request in ['hello','socket']:assert report['summary'][tool][variant][phase][request]['n']==32
    assert report['childrenExited']and report['tempClean']and report['allProbesIssuedInsidePhases']
    admit();save('WORKER.json',dict(state='passed',head=m['head'],records=records,admission=admission,resources=capped(3700000000),results_sha256=sha(out/'RESULTS.json')))
    print('CAD_INDEPENDENT_WORKER_PASS')
except BaseException:
    save('WORKER.json',dict(state='failed',head=m['head'],records=records,resources=kernel()));raise
