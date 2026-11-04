"""Qualify only the approved checkpoint fixture helper, on normal Boat."""
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
BENCH_SHA='12734a67bcdcb064618ed59eb1f23b29d6c031ad626685fd12af9ad72043307a'
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
        full,out,err=run('full-original-cases',[bun,'test/tools/store_checkpoint_io_test.cjs'])
        old=(BASE/'test/tools/store_checkpoint_io_test.cjs').read_text();new=(SOURCE/'test/tools/store_checkpoint_io_test.cjs').read_text()
        assert old.count("for(const kind of ['hash','prefix','codec'])")==1
        assert old.count('for(const after of [false,true])')==1
        expected=len(re.findall(r'\btest\(\x27',old))+2+1
        count=re.search(r'^(\d+) focused native checkpoint cases passed$',out,re.M)
        full_ok=full['exit']==0 and count is not None and int(count[1])==expected and len(re.findall(r'^ok ',out,re.M))==expected and not re.search(r'^(FAIL|Error:)',out+'\n'+err,re.M)
        # Both untouched assertion bodies must reject the same broken runtime.
        broken=OUT/'broken-runtime';shutil.copytree(SOURCE,broken)
        effect=broken/'src/server/effects/storeio.js';text=effect.read_text()
        guard="if(store_hash(path.join(home,'events.jsonl'),m.prefixBytes)!==m.prefixSha256)throw Error('prepared log prefix mismatch');"
        assert text.count(guard)==1
        effect.write_text(text.replace(guard,"if(false)throw Error('prepared log prefix mismatch');"))
        mutation_sha=digest(effect);negative=[]
        anchor="let n=0;function test(name,run){fixture(run);console.log('ok '+name);n++;}"
        replacement=r'''let n=0;function test(name,run){
try{fixture(run);console.log('ok '+name);n++;if(name==='rejected prefix offer leaves current cache untouched')process.exit(0);}
catch(error){console.error('__BP_NEGATIVE__'+JSON.stringify({case:name,name:error.name,code:error.code,operator:error.operator,
message:String(error.message).replace(/\x1b\[[0-9;]*m/g,'').replace(/[0-9a-f]{64}/g,'<sha256>')}));throw error;}}'''
        for mode,text in [('before',old),('after',new)]:
            assert text.count("path.resolve(__dirname,'../..')")==1 and text.count(anchor)==1
            modified=text.replace("path.resolve(__dirname,'../..')",json.dumps(str(broken))).replace(anchor,replacement)
            script=OUT/('negative-'+mode+'.cjs');script.write_text(modified)
            record,nout,nerr=run('reject-broken-prefix-'+mode,[bun,script])
            notices=[s[len('__BP_NEGATIVE__'):] for s in nerr.splitlines() if s.startswith('__BP_NEGATIVE__')]
            notice=json.loads(notices[-1]) if notices else None
            negative.append({'mode':mode,'execution':record,'notice':notice,'prior_ok_count':len(re.findall(r'^ok ',nout,re.M)),'script_sha256':digest(script)})
        expected_case='rejected prefix offer leaves current cache untouched'
        negative_ok=all(n['execution']['exit']!=0 and not n['execution']['timed_out'] and n['notice'] is not None
            and n['notice'].get('case')==expected_case and n['notice'].get('code')=='ERR_ASSERTION' and n['prior_ok_count']==3 for n in negative)
        if negative_ok:negative_ok=negative[0]['notice']==negative[1]['notice']
        write('NEGATIVE.json',{'mutation_sha256':mutation_sha,'removed_check':guard,'cases':negative,
            'same_failure':negative_ok,'note':'Computed DB hashes normalized in error messages; no assertions changed.'})
        parity=json.loads((OUT/'FIXTURE-PARITY.json').read_text()) if (OUT/'FIXTURE-PARITY.json').exists() else None
        identity=inventory(SOURCE)==FILES and inventory(BASE)==BASE_FILES and digest(SELF)==self_sha and digest(BENCH)==BENCH_SHA and digest(bun)==BUN_SHA and digest(node)==NODE_SHA
        after=resources()
        passed=bench['exit']==0 and parity is not None and full_ok and negative_ok and identity and after['peak_bytes']<=after['memory_max_bytes'] and all(after['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
        write('RESULT.json',{'kind':'checkpoint fixture-only qualification','source_revision':REV,'source_tree':TREE,'base_revision':BASE_REV,
            'fixture_parity':parity is not None and bench['exit']==0,'stats':parity.get('stats') if parity else None,
            'full_candidate_cases_passed':full_ok,'expected_cases':expected,'full_candidate_execution':full,
            'mutation_same_failure':negative_ok,'identity_intact':identity,'resources':after,'checkpoint_test_qualified':passed,
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
        'combined_cap_bytes':4236869632,'checkpoint_test_qualified':r.returncode==0 and result is not None and result['checkpoint_test_qualified'],
        'full_ci_proof_build_release_qualified':False})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE);shutil.rmtree(BASE)
    return 0 if r.returncode==0 and result is not None and result['checkpoint_test_qualified'] else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
