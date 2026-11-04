"""Trace the original checkpoint rejection path; never a passing CI receipt."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tarfile
import threading
import time

REV='22d5e2202204371d3ad58f052f87cd4f89501b1f'
TREE='2544558e0e87f8c7b7f35ded93e87e471e80e9aa'
ARCHIVE_SHA='97da1b2953b903239ce0bff9706730d7a8cd4767f88414326fcfbfbc466ec0af'
FILES={
 'scripts/heavy-boat.sh':('fd5510a80bd7501eda00929c6481e83e58562f2e78b8d55beb1ad30e8ae2b6a4',0o755),
 'scripts/local-phase.py':('5edf28b7f91bf938576e818cca50491b27d4a3b84c74050bc91ebf4524ef5e17',0o644),
 'src/core/storedb.bend':('f84c965550a419e952ee1bcd78d47a26922bf58ad934bd6b111089493fdc37f8',0o644),
 'src/server/effects/storeio.js':('fc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93',0o644),
 'src/server/storeio.bend':('aa6f7dee819470542174965b67a5d907a1705854b35aee28f3dd9201b360fa1a',0o644),
 'test/tools/store_checkpoint_io_test.cjs':('0d3e4b73172214b0c992d4eb41fe36f0fb5a287cd36b3738347f9b99a7d136f9',0o644),
}
SELF=Path(__file__).resolve()
ARCHIVE=SELF.with_name('CI-CHECKPOINT.tar.gz')
OUT=Path(os.environ.get('BP_CHECKPOINT_OUT',str(Path.cwd()/'build/checkpoint-diagnostic')))
SOURCE=OUT/'source'
BUN_SHA='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
NODE_SHA='d95de52ccb76fb2c5775bf176f29f3025e4b19352c092aad51ce5d930717359f'

def digest(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def inventory():
    return {p.relative_to(SOURCE).as_posix():[digest(p),p.stat().st_mode&0o777]
        for p in sorted(SOURCE.rglob('*')) if p.is_file()}

def write(name,data):
    (OUT/name).write_text(json.dumps(data,sort_keys=True,indent=2)+'\n')

def resources():
    p=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
    return {'cgroup':str(p.relative_to('/sys/fs/cgroup')),'memory_max_bytes':int((p/'memory.max').read_text()),
        'swap_max_bytes':int((p/'memory.swap.max').read_text()),'peak_bytes':int((p/'memory.peak').read_text()),
        'events':{k:int(v) for k,v in (s.split() for s in (p/'memory.events').read_text().splitlines())}}

HEADER=r'''
function __bp_mark(kind, data) {
  require('node:fs').writeSync(2, JSON.stringify({seconds:performance.now()/1000,kind,data})+'\n');
}
function __bp_args(args) {
  return args.map(v => typeof v === 'string' ? v.slice(0,180) : typeof v);
}
function __bp_proxy(object, label) {
  return new Proxy(object, {get(target,key) {
    const value = Reflect.get(target,key,target);
    if (typeof value !== 'function') return value;
    return function(...args) {
      __bp_mark('enter',{label,method:String(key),args:__bp_args(args)});
      try {
        const result = Reflect.apply(value,target,args);
        __bp_mark('return',{label,method:String(key)});
        return key === 'query' ? __bp_proxy(result,label+'.statement') : result;
      } catch(error) {
        __bp_mark('throw',{label,method:String(key),message:String(error)});
        throw error;
      }
    };
  }});
}
const __bp_real_sqlite = require('bun:sqlite');
const __bp_sqlite = {...__bp_real_sqlite, Database:new Proxy(__bp_real_sqlite.Database, {
  construct(target,args) {
    __bp_mark('enter',{label:'Database',method:'constructor',args:__bp_args(args)});
    const db=Reflect.construct(target,args,target);
    __bp_mark('return',{label:'Database',method:'constructor'});
    return __bp_proxy(db,'Database');
  }
})};
'''
WRAP=r'''
for (const name of Object.keys(c).filter(n=>/^store_/.test(n) && typeof c[n]==='function')) {
  const original=c[name];
  c[name]=function(...args) {
    __bp_mark('enter',{label:'effect',method:name,args:__bp_args(args)});
    try {return Reflect.apply(original,this,args);}
    finally {__bp_mark('leave',{label:'effect',method:name});}
  };
}
'''

def worker():
    os.chdir(SOURCE)
    cap=resources();assert cap['memory_max_bytes']==3699998720 and cap['swap_max_bytes']==0
    assert shutil.which('bend') is None
    expected={k:list(v) for k,v in FILES.items()};assert inventory()==expected
    bun=Path(shutil.which('bun')).resolve();node=Path(shutil.which('node')).resolve()
    assert digest(bun)==BUN_SHA and digest(node)==NODE_SHA
    lock=os.open('/tmp/bp-bend.lock',os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o666)
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    probe=os.open('/tmp/bp-bend.lock',os.O_RDWR|os.O_NOFOLLOW)
    try:
        try:fcntl.flock(probe,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:pass
        else:raise RuntimeError('native bend flock missing')
    finally:os.close(probe)
    text=(SOURCE/'test/tools/store_checkpoint_io_test.cjs').read_text()
    edits=[
      ("path.resolve(__dirname,'../..')",json.dumps(str(SOURCE))),
      ("const {Database}=require('bun:sqlite');","const {Database}=__bp_sqlite;"),
      ("n==='fs'?io:require(n)","n==='fs'?__bp_proxy(io,'fs'):n==='bun:sqlite'?__bp_sqlite:require(n)"),
      ('vm.runInContext(source,c);return c;','vm.runInContext(source,c);'+WRAP+'return c;'),
      ('function fixture(run){const h=',"function fixture(run){__bp_mark('fixture-enter');const h="),
      ("db(path.join(h,'store.db'),'old');run(h)","__bp_mark('fixture-db-start');db(path.join(h,'store.db'),'old');__bp_mark('fixture-db-finished');run(h);__bp_mark('case-body-finished')"),
      ("let n=0;function test(name,run){fixture(run);console.log('ok '+name);n++;}",
       "let n=0;function test(name,run){__bp_mark('CASE_BEGIN',name);fixture(run);__bp_mark('CASE_END',name);console.log('ok '+name);n++;if(name==='rejected prefix offer leaves current cache untouched'){__bp_mark('DIAGNOSTIC_CUTOFF');process.exit(0);}}"),
    ]
    for old,new in edits:
        assert text.count(old)==1,('diagnostic anchor differs',old)
        text=text.replace(old,new)
    instrumented=OUT/'instrumented.cjs';instrumented.write_text(HEADER+text)
    script_sha=digest(SELF);derived_sha=digest(instrumented)
    env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','BEND_NO_TELEMETRY':'1','BACKPLANE_TELEMETRY':'off',
        'BACKPLANE_NO_UPDATE':'1','BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_HEAVY_LOCK_FD':str(lock)}
    write('STARTED.json',{'revision':REV,'tree':TREE,'source_files':expected,'resources':cap,'actual_native_flock':True,
        'bun_sha256':digest(bun),'node_sha256':digest(node),'python':sys.version,'python_sha256':digest(sys.executable),
        'derived_test_sha256':derived_sha,'harness_sha256':script_sha,'edits':edits,
        'environment':{k:env[k] for k in ('PYTHONDONTWRITEBYTECODE','BEND_NO_TELEMETRY','BACKPLANE_TELEMETRY','BACKPLANE_NO_UPDATE','BACKPLANE_KISTACK_SYNC')},
        'limits':'Instrumented first4 actual checkpoint cases, original production bytes unchanged; never full test/CI qualification.'})
    stop=threading.Event()
    with (OUT/'test.stdout').open('wb') as stdout,(OUT/'test.stderr').open('wb') as stderr:
        process=subprocess.Popen([str(bun),str(instrumented)],cwd=SOURCE,env=env,pass_fds=(lock,),
            stdout=stdout,stderr=stderr,start_new_session=True)
        def sample():
            with (OUT/'owned-processes.jsonl').open('w') as f:
                while not stop.wait(0.5):
                    owned=[]
                    for stat in Path('/proc').glob('[0-9]*/stat'):
                        try:
                            raw=stat.read_text();fields=raw[raw.rfind(')')+2:].split()
                            if int(fields[2])!=process.pid:continue
                            owned.append({'pid':int(stat.parent.name),'state':fields[0],
                                'user_ticks':int(fields[11]),'system_ticks':int(fields[12]),'start_ticks':fields[19],
                                'rss_pages':int(fields[21]),'wchan':(stat.parent/'wchan').read_text().strip()})
                        except (OSError,ValueError,IndexError):pass
                    f.write(json.dumps({'seconds':time.monotonic(),'process_group':process.pid,'owned':owned})+'\n');f.flush()
        thread=threading.Thread(target=sample,daemon=True);thread.start()
        started=time.monotonic();timed_out=False
        try:code=process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            timed_out=True;code=124
            os.killpg(process.pid,signal.SIGTERM)
            try:process.wait(timeout=2)
            except subprocess.TimeoutExpired:pass
            try:os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError:pass
            process.wait(timeout=5)
        finally:stop.set();thread.join(2)
    after=resources()
    identity=inventory()==expected and digest(SELF)==script_sha and digest(instrumented)==derived_sha and digest(bun)==BUN_SHA and digest(node)==NODE_SHA
    write('DIAGNOSTIC.json',{'kind':'original checkpoint rejection path with tracing','revision':REV,'tree':TREE,
        'child_exit':code,'timed_out':timed_out,'seconds':time.monotonic()-started,'identity_intact':identity,
        'stdout_sha256':digest(OUT/'test.stdout'),'stderr_sha256':digest(OUT/'test.stderr'),'resources':after,
        'qualification':False,'limits':'Tracer can affect timing; four original cases only. A timeout is not a passing test. No runtime behavior patch.'})
    return 0 if identity else 1

def controller():
    assert resources()['memory_max_bytes']==536870912 and resources()['swap_max_bytes']==0
    assert not OUT.exists(),'already started; inspect rather than restart'
    OUT.mkdir(parents=True);assert digest(ARCHIVE)==ARCHIVE_SHA
    SOURCE.mkdir()
    with tarfile.open(ARCHIVE) as tar:tar.extractall(SOURCE,filter='data')
    assert inventory()=={k:list(v) for k,v in FILES.items()}
    unit='bp-checkpoint-trace-'+str(os.getpid())+'.scope'
    timed_out=False
    try:
        with (OUT/'worker.log').open('w') as log:
            r=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000','-p','MemorySwapMax=0',
                '--','sh','scripts/heavy-boat.sh',sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,
                env={**os.environ,'BP_CHECKPOINT_OUT':str(OUT)},stdout=log,stderr=subprocess.STDOUT,timeout=150)
    except subprocess.TimeoutExpired:
        timed_out=True;subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20);r=subprocess.CompletedProcess([],124)
    diagnostic=json.loads((OUT/'DIAGNOSTIC.json').read_text()) if (OUT/'DIAGNOSTIC.json').exists() else None
    write('SUMMARY.json',{'worker_exit':r.returncode,'controller_timed_out':timed_out,'diagnostic':diagnostic,
        'controller':resources(),'combined_cap_bytes':4236869632,'qualification':False,'archive_sha256':digest(ARCHIVE)})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE)
    return 0 if r.returncode==0 and diagnostic is not None else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
