"""One admitted real SQLite promotion-fixture before/after, not a CI gate."""
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

REV='7e1d5ddbf80729ae14f94887076c09688b005c51'
TREE='10a565d6bea7195565106c6a4d7ce58c862832e0'
BASE_REV='0c63ed2629e3d888ae56766e21de606d9f1a651a'
BASE_TAR_SHA='f5bc38724d31d3642d5d1daf42d6e98bc294366a1807c57f1d9e3dba18c03923'
CAND_TAR_SHA='3266f1ff476011965e8dbb458b1c776eb0a1d2213519a40c8a929a5425d1ce11'
BENCH_SHA='58218abf7a31b8cef6b1532b273cfd8ab73bb14b5f9c44546c3bdaf436a91cf8'
BUN_SHA='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
NODE_SHA='d95de52ccb76fb2c5775bf176f29f3025e4b19352c092aad51ce5d930717359f'
FILES={
 'scripts/heavy-boat.sh':['fd5510a80bd7501eda00929c6481e83e58562f2e78b8d55beb1ad30e8ae2b6a4',0o755],
 'scripts/local-phase.py':['5edf28b7f91bf938576e818cca50491b27d4a3b84c74050bc91ebf4524ef5e17',0o644],
 'src/core/storedb.bend':['f84c965550a419e952ee1bcd78d47a26922bf58ad934bd6b111089493fdc37f8',0o644],
 'src/server/effects/db.js':['1472da9b675e8b0d9badc4da87a36f339865d9c008ea27220875ac77661c4f40',0o644],
 'src/server/effects/storeio.js':['fc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93',0o644],
 'test/tools/store_promote_test.cjs':['e86eb84fb09cf6c48e40d7277b55e8e30693b44917f89436726f1eecfeabc699',0o644],
}
BASE_FILES={**FILES,'test/tools/store_promote_test.cjs':['193c3a453ee3a0e105bef7fd58d7a20629223ddfc830aa440e14130e830a494b',0o644]}
SELF=Path(__file__).resolve();BENCH=SELF.with_suffix('.cjs')
OUT=Path(os.environ.get('BP_PROMOTE_OUT',str(Path.cwd()/'build/promote-fixture-qual')))
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
    env.pop('STOREIO_SOURCE',None)
    write('STARTED.json',{'source_revision':REV,'source_tree':TREE,'base_revision':BASE_REV,'files':FILES,'base_files':BASE_FILES,
        'harness_sha256':self_sha,'bench_sha256':BENCH_SHA,'bun_sha256':digest(bun),'node_sha256':digest(node),
        'bun_version':subprocess.check_output([str(bun),'--version'],text=True).strip(),
        'node_version':subprocess.check_output([str(node),'--version'],text=True).strip(),
        'python':sys.version,'resources':before,'actual_native_flock':True,
        'filesystem':subprocess.check_output(['stat','-f','-c','%T',str(OUT)],text=True).strip(),
        'environment':{k:env[k] for k in ('PYTHONDONTWRITEBYTECODE','BEND_NO_TELEMETRY','BACKPLANE_TELEMETRY','BACKPLANE_NO_UPDATE','BACKPLANE_KISTACK_SYNC','NO_COLOR')},
        'limits':'Test fixture only; no native probe, runtime edit, compiler, full CI or release qualification.'})
    records=[]
    def run(label,args,cwd,timeout=40,extra_env=None):
        tm=OUT/(label+'.time');stdout=OUT/(label+'.stdout');stderr=OUT/(label+'.stderr')
        began=time.monotonic();expired=False;unkillable=False
        with stdout.open('wb') as out,stderr.open('wb') as err:
            p=subprocess.Popen(['/usr/bin/time','-f','{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,"system_seconds":%S}',
                '-o',str(tm),*map(str,args)],cwd=cwd,env={**env,**(extra_env or {})},pass_fds=(lock,),stdout=out,stderr=err,start_new_session=True)
            try:code=p.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                expired=True;code=124
                try:os.killpg(p.pid,signal.SIGTERM)
                except ProcessLookupError:pass
                try:p.wait(timeout=2)
                except subprocess.TimeoutExpired:pass
                try:os.killpg(p.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:unkillable=True
        metrics=next((json.loads(s) for s in reversed(tm.read_text().splitlines()) if s.startswith('{')), {}) if tm.exists() else {}
        record={'label':label,'command':list(map(str,args)),'cwd':str(cwd),'exit':code,'timed_out':expired,'unkillable':unkillable,
            'parent_elapsed_seconds':time.monotonic()-began,**metrics,'stdout_sha256':digest(stdout),'stderr_sha256':digest(stderr)}
        records.append(record);write('COMMANDS.json',records)
        return record,stdout.read_text(errors='replace'),stderr.read_text(errors='replace')
    def suite_ok(record,out,err):
        summaries=[json.loads(s) for s in out.splitlines() if s.startswith('{')]
        return record['exit']==0 and not record['timed_out'] and len(summaries)==1 and summaries[0].get('passed')==11 and summaries[0].get('failed')==0 and len(re.findall(r'^ok ',out,re.M))==11 and not re.search(r'^FAIL ',err,re.M)
    try:
        pairs=[]
        for trial in range(3):
            pair={}
            for mode in (['after','before'] if trial%2 else ['before','after']):
                record,out,err=run('whole-'+str(trial)+'-'+mode,[bun,'test/tools/store_promote_test.cjs'],BASE if mode=='before' else SOURCE)
                pair[mode]={'execution':record,'all_11_original_cases':suite_ok(record,out,err)}
            pairs.append(pair)
        bench,_,_=run('fixture-parity',[bun,BENCH,BASE,SOURCE,OUT],SOURCE,timeout=80)
        # Remove only the real staged-sidecar fsync. Both original suites must detect it.
        effect=(SOURCE/'src/server/effects/storeio.js').read_text()
        guard="for(const suffix of sidecars){const saved=store_sync(next+suffix);if(saved!=='ok')return saved;}"
        assert effect.count(guard)==1
        mutation=OUT/'broken-storeio.js';mutation.write_text(effect.replace(guard,'/* deliberately omitted staged-sidecar sync */'))
        negative=[]
        for mode,cwd in [('before',BASE),('after',SOURCE)]:
            record,out,err=run('reject-missing-sidecar-sync-'+mode,[bun,'test/tools/store_promote_test.cjs'],cwd,extra_env={'STOREIO_SOURCE':str(mutation)})
            notices=re.findall(r'^FAIL (.*?)(?:\r)?$',err,re.M)
            negative.append({'mode':mode,'execution':record,'failures':notices,'detected':record['exit']!=0 and not record['timed_out'] and any(s.startswith('WAL is durable before any current store is moved:') for s in notices)})
        parity=json.loads((OUT/'FIXTURE-PARITY.json').read_text()) if (OUT/'FIXTURE-PARITY.json').exists() else None
        identity=inventory(SOURCE)==FILES and inventory(BASE)==BASE_FILES and digest(SELF)==self_sha and digest(BENCH)==BENCH_SHA and digest(bun)==BUN_SHA and digest(node)==NODE_SHA
        after=resources()
        passed=all(p[m]['all_11_original_cases'] for p in pairs for m in ('before','after')) and bench['exit']==0 and parity is not None and all(n['detected'] for n in negative) and identity and after['peak_bytes']<=after['memory_max_bytes'] and all(after['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
        write('RESULT.json',{'kind':'promotion fixture-only qualification','source_revision':REV,'source_tree':TREE,'base_revision':BASE_REV,
            'whole_suite_pairs':pairs,'fixture_parity':parity is not None and bench['exit']==0,'fixture_stats':parity.get('stats') if parity else None,
            'negative_controls':negative,'mutation_sha256':digest(mutation),'identity_intact':identity,'resources':after,
            'promotion_test_qualified':passed,'full_ci_proof_build_release_qualified':False,
            'limits':'Three alternating complete-suite pairs and five open-helper setup pairs on one VM. Open timing excludes close. No checkpoint-timeout cause or whole CI speedup claim.'})
        return 0 if passed else 1
    finally:
        write('FINAL-IDENTITY.json',{'source_unchanged':inventory(SOURCE)==FILES,'reference_unchanged':inventory(BASE)==BASE_FILES,
            'harness_unchanged':digest(SELF)==self_sha,'bench_unchanged':digest(BENCH)==BENCH_SHA,
            'bun_unchanged':digest(bun)==BUN_SHA,'node_unchanged':digest(node)==NODE_SHA,'resources':resources()})

def controller():
    assert resources()['memory_max_bytes']==536870912 and resources()['swap_max_bytes']==0
    assert not OUT.exists(),'already started; inspect instead of repeating'
    OUT.mkdir(parents=True)
    for name,dest,sha,files in [('ZCI-PROMOTE-BASE.tar.gz',BASE,BASE_TAR_SHA,BASE_FILES),('ZCI-PROMOTE-CAND.tar.gz',SOURCE,CAND_TAR_SHA,FILES)]:
        archive=SELF.with_name(name);assert digest(archive)==sha
        dest.mkdir()
        with tarfile.open(archive) as tar:tar.extractall(dest,filter='data')
        assert inventory(dest)==files
    assert digest(BENCH)==BENCH_SHA
    unit='bp-promote-qual-'+str(os.getpid())+'.scope';expired=False
    try:
        with (OUT/'worker.log').open('w') as log:
            r=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000','-p','MemorySwapMax=0',
                '--','sh','scripts/heavy-boat.sh',sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,
                env={**os.environ,'BP_PROMOTE_OUT':str(OUT)},stdout=log,stderr=subprocess.STDOUT,timeout=640)
    except subprocess.TimeoutExpired:
        expired=True;subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20);r=subprocess.CompletedProcess([],124)
    result=json.loads((OUT/'RESULT.json').read_text()) if (OUT/'RESULT.json').exists() else None
    write('SUMMARY.json',{'worker_exit':r.returncode,'controller_timed_out':expired,'result':result,'controller':resources(),
        'combined_cap_bytes':4236869632,'promotion_test_qualified':r.returncode==0 and result is not None and result['promotion_test_qualified'],
        'full_ci_proof_build_release_qualified':False})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE);shutil.rmtree(BASE)
    return 0 if r.returncode==0 and result is not None and result['promotion_test_qualified'] else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
