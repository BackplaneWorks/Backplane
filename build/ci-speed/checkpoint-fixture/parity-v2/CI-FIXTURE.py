"""Remaining fixture parity/rollback measurement only; reuse completed cases."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import time

REV='e6b558e1627f27878f028f149613b160bc85b117'
TREE='791446bf839f30f2fb4db9bdc698a874c834fe7a'
BASE_REV='22d5e2202204371d3ad58f052f87cd4f89501b1f'
BASE_TAR_SHA='97da1b2953b903239ce0bff9706730d7a8cd4767f88414326fcfbfbc466ec0af'
CAND_TAR_SHA='f57a907440c1ca033d9c860a80fd754e55658805c1138d43f275fa65b0181e1f'
BENCH_SHA='5f591c33cbc9dbd0c17bc144d2f393c875cc473247269b2ff171724d13992df2'
BUN_SHA='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
NODE_SHA='d95de52ccb76fb2c5775bf176f29f3025e4b19352c092aad51ce5d930717359f'
FILES={
 'scripts/heavy-boat.sh':['fd5510a80bd7501eda00929c6481e83e58562f2e78b8d55beb1ad30e8ae2b6a4',0o755],
 'scripts/local-phase.py':['5edf28b7f91bf938576e818cca50491b27d4a3b84c74050bc91ebf4524ef5e17',0o644],
 'src/core/storedb.bend':['f84c965550a419e952ee1bcd78d47a26922bf58ad934bd6b111089493fdc37f8',0o644],
 'src/server/effects/storeio.js':['fc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93',0o644],
 'src/server/storeio.bend':['aa6f7dee819470542174965b67a5d907a1705854b35aee28f3dd9201b360fa1a',0o644],
 'test/tools/store_checkpoint_io_test.cjs':['70675f2307ae9c11308d36082e25876e5a9d8e92bb41b6be753a9b30ce0e2c6e',0o644],
}
BASE_FILES={**FILES,'test/tools/store_checkpoint_io_test.cjs':['0d3e4b73172214b0c992d4eb41fe36f0fb5a287cd36b3738347f9b99a7d136f9',0o644]}
SELF=Path(__file__).resolve();BENCH=SELF.with_suffix('.cjs')
OUT=Path(os.environ.get('BP_FIXTURE_OUT',str(Path.cwd()/'build/checkpoint-fixture-qual')))
SOURCE=OUT/'candidate';BASE=OUT/'reference'

def digest(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def inventory(root):
    return {p.relative_to(root).as_posix():[digest(p),p.stat().st_mode&0o777] for p in sorted(root.rglob('*')) if p.is_file()}

def write(name,data):
    (OUT/name).write_text(json.dumps(data,sort_keys=True,indent=2)+'\n')

def resources():
    p=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
    return {'cgroup':str(p.relative_to('/sys/fs/cgroup')),'memory_max_bytes':int((p/'memory.max').read_text()),
        'swap_max_bytes':int((p/'memory.swap.max').read_text()),'peak_bytes':int((p/'memory.peak').read_text()),
        'events':{k:int(v) for k,v in (s.split() for s in (p/'memory.events').read_text().splitlines())}}

def worker():
    os.chdir(SOURCE)
    before=resources();assert before['memory_max_bytes']==3699998720 and before['swap_max_bytes']==0
    assert inventory(SOURCE)==FILES and inventory(BASE)==BASE_FILES
    assert shutil.which('bend') is None
    bun=Path(shutil.which('bun')).resolve();node=Path(shutil.which('node')).resolve()
    assert digest(bun)==BUN_SHA and digest(node)==NODE_SHA and digest(BENCH)==BENCH_SHA
    self_sha=digest(SELF)
    lock=os.open('/tmp/bp-bend.lock',os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o666)
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    probe=os.open('/tmp/bp-bend.lock',os.O_RDWR|os.O_NOFOLLOW)
    try:
        try:fcntl.flock(probe,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:pass
        else:raise RuntimeError('native flock not held')
    finally:os.close(probe)
    env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','BEND_NO_TELEMETRY':'1','BACKPLANE_TELEMETRY':'off',
        'BACKPLANE_NO_UPDATE':'1','BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_HEAVY_LOCK_FD':str(lock),'NO_COLOR':'1'}
    write('STARTED.json',{'source_revision':REV,'source_tree':TREE,'base_revision':BASE_REV,'files':FILES,'base_files':BASE_FILES,
        'harness_sha256':self_sha,'bench_sha256':BENCH_SHA,'bun_sha256':digest(bun),'node_sha256':digest(node),
        'bun_version':subprocess.check_output([str(bun),'--version'],text=True).strip(),
        'node_version':subprocess.check_output([str(node),'--version'],text=True).strip(),
        'python':sys.version,'resources':before,'actual_native_flock':True,
        'filesystem':subprocess.check_output(['stat','-f','-c','%T',str(OUT)],text=True).strip(),
        'environment':{k:env[k] for k in ('PYTHONDONTWRITEBYTECODE','BEND_NO_TELEMETRY','BACKPLANE_TELEMETRY','BACKPLANE_NO_UPDATE','BACKPLANE_KISTACK_SYNC','NO_COLOR')},
        'limits':'Test-only fixture construction and checkpoint suite, no runtime change or full CI/proof/build qualification.'})
    records=[]
    def run(label,args,timeout=60):
        tm=OUT/(label+'.time');stdout=OUT/(label+'.stdout');stderr=OUT/(label+'.stderr')
        began=time.monotonic();expired=False
        with stdout.open('wb') as out,stderr.open('wb') as err:
            p=subprocess.Popen(['/usr/bin/time','-f','{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,"system_seconds":%S}',
                '-o',str(tm),*map(str,args)],cwd=SOURCE,env=env,pass_fds=(lock,),stdout=out,stderr=err,start_new_session=True)
            try:code=p.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                expired=True;code=124;os.killpg(p.pid,signal.SIGTERM)
                try:p.wait(timeout=2)
                except subprocess.TimeoutExpired:pass
                try:os.killpg(p.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                p.wait(timeout=5)
        metrics=next((json.loads(s) for s in reversed(tm.read_text().splitlines()) if s.startswith('{')), {}) if tm.exists() else {}
        record={'label':label,'command':list(map(str,args)),'exit':code,'timed_out':expired,
            'parent_elapsed_seconds':time.monotonic()-began,**metrics,'stdout_sha256':digest(stdout),'stderr_sha256':digest(stderr)}
        records.append(record);write('COMMANDS.json',records)
        return record,stdout.read_text(errors='replace'),stderr.read_text(errors='replace')
    try:
        bench,_,_=run('fixture-parity',[bun,BENCH,BASE,SOURCE,OUT])
        # Full 25-case execution and both original/candidate mutation controls
        # completed in owned job j20261104063951-69873. Do not replay them or
        # import a declared pass: this worker reports its remaining scope only.
        parity=json.loads((OUT/'FIXTURE-PARITY.json').read_text()) if (OUT/'FIXTURE-PARITY.json').exists() else None
        identity=inventory(SOURCE)==FILES and inventory(BASE)==BASE_FILES and digest(SELF)==self_sha and digest(BENCH)==BENCH_SHA and digest(bun)==BUN_SHA and digest(node)==NODE_SHA
        after=resources()
        passed=bench['exit']==0 and parity is not None and identity and after['peak_bytes']<=after['memory_max_bytes'] and all(after['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
        write('RESULT.json',{'kind':'remaining fixture parity/rollback measurement','source_revision':REV,'source_tree':TREE,'base_revision':BASE_REV,
            'fixture_parity':parity is not None and bench['exit']==0,'stats':parity.get('stats') if parity else None,
            'full_cases_executed_this_job':0,'mutation_cases_executed_this_job':0,
            'previous_owned_full_case_job':'j20261104063951-69873',
            'identity_intact':identity,'resources':after,'fixture_parity_qualified':passed,
            'checkpoint_test_qualified':False,
            'full_ci_proof_build_release_qualified':False,'original_90s_timeout_cause_identified':False,
            'limits':'Five alternating fixture setup pairs in one VM. Original 90s batch not replayed; no runtime or durability behavior change.'})
        return 0 if passed else 1
    finally:
        write('FINAL-IDENTITY.json',{'source_unchanged':inventory(SOURCE)==FILES,'reference_unchanged':inventory(BASE)==BASE_FILES,
            'harness_unchanged':digest(SELF)==self_sha,'bench_unchanged':digest(BENCH)==BENCH_SHA,
            'bun_unchanged':digest(bun)==BUN_SHA,'node_unchanged':digest(node)==NODE_SHA,'resources':resources()})

def controller():
    assert resources()['memory_max_bytes']==536870912 and resources()['swap_max_bytes']==0
    assert not OUT.exists(),'already started; inspect instead of repeating'
    OUT.mkdir(parents=True)
    for name,dest,sha,files in [('CI-BASE.tar.gz',BASE,BASE_TAR_SHA,BASE_FILES),('CI-CAND.tar.gz',SOURCE,CAND_TAR_SHA,FILES)]:
        archive=SELF.with_name(name);assert digest(archive)==sha
        dest.mkdir()
        with tarfile.open(archive) as tar:tar.extractall(dest,filter='data')
        assert inventory(dest)==files
    assert digest(BENCH)==BENCH_SHA
    unit='bp-fixture-qual-'+str(os.getpid())+'.scope'
    expired=False
    try:
        with (OUT/'worker.log').open('w') as log:
            r=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000','-p','MemorySwapMax=0',
                '--','sh','scripts/heavy-boat.sh',sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,
                env={**os.environ,'BP_FIXTURE_OUT':str(OUT)},stdout=log,stderr=subprocess.STDOUT,timeout=240)
    except subprocess.TimeoutExpired:
        expired=True;subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20);r=subprocess.CompletedProcess([],124)
    result=json.loads((OUT/'RESULT.json').read_text()) if (OUT/'RESULT.json').exists() else None
    write('SUMMARY.json',{'worker_exit':r.returncode,'controller_timed_out':expired,'result':result,'controller':resources(),
        'combined_cap_bytes':4236869632,'fixture_parity_qualified':r.returncode==0 and result is not None and result['fixture_parity_qualified'],
        'checkpoint_test_qualified':False,
        'full_ci_proof_build_release_qualified':False})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE);shutil.rmtree(BASE)
    return 0 if r.returncode==0 and result is not None and result['fixture_parity_qualified'] else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
