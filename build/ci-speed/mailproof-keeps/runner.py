"""One current canonical MailProof.keeps shard through the exact saved #877 loader."""
import fcntl
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tarfile
import threading
import time

REV='957a50435fe3e018e4397746f6e0b10dd42828d4'
TREE='bcabcceded6d7d00079f5552ff0d054202816e90'
BUNDLE='bc883722146564d29c05cc1971a2ac09f55fbc98b65a835ea0089b0551ff62c7'
META='6fcbe29b7e0e1a3376b78350fe1c996fd9cb04fb380aba04c7b49891d1cfbf4e'
BUN='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
SELF=Path(__file__).resolve()
OUT=Path(os.environ.get('BP_MAIL_OUT',str(Path.cwd()/'build/mailproof-repro')))
SOURCE=OUT/'source'
PLAN=SOURCE/'build/proof-shards/mailproof'
COMPILER=OUT/'loader/compiler/bend2'


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda:f.read(65536),b''):h.update(chunk)
    return h.hexdigest()


def write(name,value):
    (OUT/name).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')


def load(path,name):
    spec=importlib.util.spec_from_file_location(name,path)
    mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
    return mod


def group():
    return Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')


def resources(at):
    return {'cgroup':str(at.relative_to('/sys/fs/cgroup')),
            'memory_max_bytes':int((at/'memory.max').read_text()),'swap_max_bytes':int((at/'memory.swap.max').read_text()),
            'peak_bytes':int((at/'memory.peak').read_text()),
            'events':{k:int(v) for k,v in (line.split() for line in (at/'memory.events').read_text().splitlines())}}


def inputs():
    return {p.relative_to(SOURCE).as_posix():sha(p) for p in sorted(SOURCE.rglob('*')) if p.is_file()}


def compiler_identity():
    meta=OUT/'loader/CHECKER.json'
    assert sha(meta)==META
    data=json.loads(meta.read_text())
    assert sha(OUT/'loader/CHECKER.patch')==data['patch_sha256']=='4405d272062f81d713743aab104af0c5943888b65dfd33d124b66492f7340d12'
    for name,digest in data['patched_sha256'].items():
        assert sha(OUT/'loader/compiler'/name)==digest,name
    return data


def worker():
    os.chdir(SOURCE)
    at=group();before=resources(at)
    assert before['memory_max_bytes']==3699998720 and before['swap_max_bytes']==0
    guard=load(SOURCE/'scripts/heavy-guard.py','mail_actual_guard')
    assert guard.verify_shard_cgroup() and guard.admit('proof-shard'),'native bounded-shard resource admission refused'
    fd=os.open(guard.lock_path(),os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o666)
    fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);os.environ[guard.FD_VAR]=str(fd)
    assert guard.inherited_lock(guard.lock_path())==fd
    data=compiler_identity()
    bun=Path.home()/'.local/backplane-ci/bin/bun'
    assert sha(bun)==BUN
    selection=json.loads((OUT/'SELECTION.json').read_text())
    original=json.loads((OUT/'INPUTS.json').read_text())
    assert inputs()==original['files']
    ps=load(SOURCE/'scripts/proof-shards.py','mail_current_shards')
    manifest=ps.verify(SOURCE,PLAN)
    assert sha(PLAN/'manifest.json')==selection['manifest_sha256']
    entry=PLAN/selection['shard']/'PROOF.bend'
    env={**os.environ,'BEND_NO_TELEMETRY':'1','BP_BEND_MODULES':'1','BP_BEND_PAGED':'1',
         'BP_BEND_COMPACT':'0','BP_BEND_EVENT_BATCH':'128','BP_BEND_SOURCE_BODIES':'1',
         'BP_MODULE_BASELINE_CHECKER_SHA256':data['patched_sha256']['bend2/bend.ts'],
         'BUN_JSC_forceRAMSize':str(1024**3),'BUN_JSC_maxPerThreadStackUsage':str(512*1024**2),
         'BUN_JSC_numberOfGCMarkers':'1','BUN_JSC_numberOfDFGCompilerThreads':'1','BUN_JSC_numberOfFTLCompilerThreads':'1',
         'TMPDIR':str(OUT/'spill'),'BP_COMPILER_PROFILE':str(OUT/'profile.jsonl')}
    (OUT/'spill').mkdir()
    filesystem=subprocess.check_output(['stat','-f','-c','%T',str(OUT/'spill')],text=True).strip()
    assert filesystem not in ('tmpfs','ramfs')
    command=[str(bun),str(COMPILER/'main.ts'),str(entry),'--check-only']
    # The kernel is unchanged; keep an explicit typed false-theorem control
    # separate from the real canonical trace, never substitute it for the law.
    with (OUT/'bend-guide.txt').open('wb') as guide:
        subprocess.run(['bend','guide'],stdout=guide,stderr=subprocess.STDOUT,check=True,timeout=30)
    negative=OUT/'negative.bend'
    negative.write_text('import Base\ndef bad() -> {0n == 1n : Nat}:\n  {==}\n')
    start_negative=time.monotonic()
    with (OUT/'negative.log').open('wb') as log:
        rejected=subprocess.run([str(bun),str(COMPILER/'main.ts'),str(negative),'--check-only'],cwd=SOURCE,
            env={**env,'BP_COMPILER_PROFILE':str(OUT/'negative-profile.jsonl')},stdout=log,stderr=subprocess.STDOUT,
            pass_fds=(fd,),timeout=30)
    negative_text=(OUT/'negative.log').read_text()
    negative_ok=(rejected.returncode==1 and '- expected : 0n' in negative_text
                 and '- observed : 1n' in negative_text and 'Location: bad' in negative_text)
    write('NEGATIVE.json',{'compiler_exit':rejected.returncode,'typed_false_theorem_rejected':negative_ok,
          'seconds':time.monotonic()-start_negative,'log_sha256':sha(OUT/'negative.log'),'resources':resources(at)})
    assert negative_ok,'typed false theorem was not rejected at its proof body; real shard not executed'
    write('STARTED.json',{'revision':REV,'tree':TREE,'selection':selection,'resources':before,'pid':os.getpid(),
          'pid_start':Path('/proc/self/stat').read_text().split(') ')[1].split()[19],
          'command':command,'bun_sha256':sha(bun),'compiler_files':data['patched_sha256'],
          'actual_native_bend_flock':True,'native_shard_resource_admitted':True,'spill_filesystem':filesystem,
          'environment':{k:v for k,v in env.items() if k.startswith(('BP_BEND_','BP_MODULE_','BUN_JSC_','BEND_NO_'))}})
    started=time.monotonic()
    with (OUT/'checker.log').open('wb') as log:
        result=subprocess.run(['/usr/bin/time','-v','-o',str(OUT/'checker.time'),*command],cwd=SOURCE,env=env,
                              stdout=log,stderr=subprocess.STDOUT,pass_fds=(fd,))
    elapsed=time.monotonic()-started
    raw=(OUT/'checker.log').read_text()
    verdict=ps.verdict(result.returncode,raw)
    unchanged=inputs()==original['files'] and sha(bun)==BUN
    compiler_identity()
    coverage=None;coverage_error=None
    try:
        trace=load(SOURCE/'build/ci-speed/proof-source-backend-review/review.py','mail_saved_trace_audit')
        _,blocks=ps.parse(entry.read_text(),'PROOF.bend')
        expected={b['name'].replace('Laws.','LAWS:',1) if b['name'].startswith('Laws.') else b['name'] for b in blocks if b['kind']=='def'}
        with (OUT/'profile.jsonl').open() as f:
            coverage=trace.trace_coverage((json.loads(line) for line in f),expected,str(entry))
        assert 'MailProof.keeps' in expected
    except (AssertionError,OSError,ValueError,KeyError) as error:coverage_error=str(error)
    after=resources(at)
    qualified=verdict and unchanged and coverage is not None and all(after['events'].get(k)==0 for k in ('oom','oom_kill','oom_group_kill'))
    write('EXECUTION.json',{'compiler_exit':result.returncode,'repository_verdict_accepted':verdict,'seconds':elapsed,
          'peak_rss_kib':resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,'resources_before':before,'resources_after':after,
          'source_tool_unchanged':unchanged,'log_sha256':sha(OUT/'checker.log'),'trace_coverage':coverage,'trace_error':coverage_error,
          'actual_native_bend_flock':guard.inherited_lock(guard.lock_path())==fd,'qualified':qualified})
    return 0 if qualified else 1


def controller():
    at=group();state=resources(at)
    assert state['memory_max_bytes']==536870912 and state['swap_max_bytes']==0
    assert not OUT.exists(),'existing attempt; inspect instead of rerun'
    OUT.mkdir(parents=True)
    repo=OUT/'fetch.git'
    subprocess.run(['git','init','-q','--bare',str(repo)],check=True)
    def git(*args):
        return subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=0','-c','pack.threads=1',
            '-c','pack.windowMemory=32m','-C',str(repo),*args],stderr=subprocess.PIPE,timeout=120)
    git('remote','add','origin','https://github.com/BackplaneWorks/Backplane.git')
    git('config','remote.origin.promisor','true');git('config','remote.origin.partialclonefilter','blob:none')
    git('fetch','--filter=blob:none','--no-tags','origin',REV)
    assert git('rev-parse','FETCH_HEAD').decode().strip()==REV and git('rev-parse',REV+'^{tree}').decode().strip()==TREE
    archive=OUT/'source.tar'
    with archive.open('wb') as f:
        subprocess.run(['git','-c','gc.auto=0','-C',str(repo),'archive',REV,'src','test','scripts','LAWS.bend','PROOF.bend',
                        '.github/actions/setup-bend','build/ci-speed/proof-source-backend-review/review.py'],
                       stdout=f,stderr=subprocess.PIPE,check=True,timeout=120)
    SOURCE.mkdir()
    with tarfile.open(archive) as tar:tar.extractall(SOURCE,filter='data')
    archive.unlink();shutil.rmtree(repo)
    bundle=SELF.parent/'compiler.tar.gz'
    assert sha(bundle)==BUNDLE
    (OUT/'loader').mkdir()
    with tarfile.open(bundle) as tar:tar.extractall(OUT/'loader',filter='data')
    compiler_identity()
    with (OUT/'plan.log').open('w') as log:
        subprocess.run([sys.executable,'-IB','scripts/proof-shards.py','generate','--root',str(SOURCE),'--out',str(PLAN),'--shards','128'],
                       cwd=SOURCE,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=120)
    ps=load(SOURCE/'scripts/proof-shards.py','mail_plan_inspection')
    manifest=ps.verify(SOURCE,PLAN)
    found=[s for s in manifest['shards'] if ['PROOF.bend','MailProof.keeps'] in s['declarations']]
    write('CANDIDATES.json',{'target':['PROOF.bend','MailProof.keeps'],'shards':[s['id'] for s in found],
          'policy':'first declaration-bearing canonical shard, exactly as the lane3 handoff selector; helper sharing is allowed'})
    assert found,'target absent from current canonical plan'
    selected=found[0];block_hashes={}
    for origin in ('LAWS.bend','PROOF.bend'):
        _,all_blocks=ps.parse((SOURCE/origin).read_text(),origin)
        _,emitted=ps.parse((PLAN/selected['id']/origin).read_text(),origin)
        kept={tuple(k) for k in selected['declarations']}
        assert emitted==[b for b in all_blocks if (origin,b['name']) in kept], 'original canonical block changed'
        if origin=='PROOF.bend':
            assert {b['kind'] for b in emitted if b['name']=='MailProof.keeps'}=={'law','def'}, 'complete original helper law/fill required'
        block_hashes[origin]=[{'kind':b['kind'],'name':b['name'],'sha256':hashlib.sha256(b['text'].encode()).hexdigest()} for b in emitted]
    selection={'revision':REV,'shard':selected['id'],'manifest_sha256':sha(PLAN/'manifest.json'),
               'candidate_shards':[s['id'] for s in found],'policy':'first declaration-bearing canonical shard; execute only this one',
               'target':['PROOF.bend','MailProof.keeps'],'primary_laws':selected['primary_laws'],
               'declarations':selected['declarations'],'source_files':selected['source_files'],'original_blocks':block_hashes}
    write('SELECTION.json',selection);shutil.copyfile(PLAN/'manifest.json',OUT/'MANIFEST.json')
    write('INPUTS.json',{'revision':REV,'tree':TREE,'files':inputs(),'loader_bundle_sha256':BUNDLE,'runner_sha256':sha(SELF)})
    unit='bp-mailproof-'+str(os.getpid())+'.scope';stop=threading.Event()
    def sample():
        with (OUT/'memory.jsonl').open('w') as f:
            while not stop.wait(0.1):
                try:
                    started=json.loads((OUT/'STARTED.json').read_text())
                    f.write(json.dumps(resources(Path('/sys/fs/cgroup')/started['resources']['cgroup']))+'\n');f.flush()
                except (OSError,ValueError,KeyError):pass
    thread=threading.Thread(target=sample,daemon=True);thread.start();timed_out=False
    try:
        with (OUT/'worker.log').open('w') as log:
            result=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000',
                '-p','MemorySwapMax=0','--','sh','scripts/heavy-boat.sh',sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,
                env={**os.environ,'BP_MAIL_OUT':str(OUT)},stdout=log,stderr=subprocess.STDOUT,timeout=1100)
    except subprocess.TimeoutExpired:
        timed_out=True;subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20)
        result=subprocess.CompletedProcess([],124)
    finally:stop.set();thread.join(2)
    journal=subprocess.run(['journalctl','--user','--unit='+unit,'--no-pager','-n','80'],capture_output=True,timeout=20)
    (OUT/'unit-journal.txt').write_bytes(journal.stdout+journal.stderr)
    status=subprocess.run(['systemctl','--user','show',unit,'-p','Result','-p','ActiveState','-p','MemoryMax','-p','MemorySwapMax'],capture_output=True,timeout=20)
    (OUT/'unit-status-diagnostic.txt').write_bytes(status.stdout+status.stderr)
    execution=json.loads((OUT/'EXECUTION.json').read_text()) if (OUT/'EXECUTION.json').exists() else None
    write('SUMMARY.json',{'revision':REV,'tree':TREE,'selection':selection,'worker_unit_exit':result.returncode,
          'timed_out':timed_out,'execution':execution,'controller':resources(at),'combined_cap_bytes':4236869632,
          'qualified_scoped_experiment':execution is not None and execution['qualified'] and result.returncode==0,
          'full_proof_qualified':False,'cache_pass_records_created':0,'build_release_qualified':False,
          'controls':'Exact #877/#856 loader identity and saved 10 trace controls; fresh typed false theorem rejected before candidate canonical shard',
          'limits':'One original canonical shard only; experimental loader, not stock binary. Missing inner receipt is inconclusive.'})
    profile=OUT/'profile.jsonl'
    if profile.exists():
        write('PROFILE-IDENTITY.json',{'uncompressed_sha256':sha(profile)})
        with profile.open('rb') as src,gzip.open(OUT/'profile.jsonl.gz','wb') as dst:shutil.copyfileobj(src,dst)
        profile.unlink()
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE);shutil.rmtree(OUT/'loader')
    if (OUT/'spill').exists():shutil.rmtree(OUT/'spill')
    return 0 if execution is not None and execution['qualified'] and result.returncode==0 else 1


if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
