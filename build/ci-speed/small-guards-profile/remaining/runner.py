"""Profile ONLY the five previously unexecuted frozen guard commands on Boat."""
import ast
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
import time

REV='22d5e2202204371d3ad58f052f87cd4f89501b1f'
TREE='2544558e0e87f8c7b7f35ded93e87e471e80e9aa'
SELF=Path(__file__).resolve()
OUT=Path(os.environ.get('BP_SMALL_OUT',str(Path.cwd()/'build/remaining-guards-profile')))
SOURCE=OUT/'source'
BUN_SHA='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
NODE_SHA='d95de52ccb76fb2c5775bf176f29f3025e4b19352c092aad51ce5d930717359f'

def network_bytes():
    return {line.split(':',1)[0].strip():{'received':int(line.split(':',1)[1].split()[0]),
        'sent':int(line.split(':',1)[1].split()[8])}
        for line in Path('/proc/net/dev').read_text().splitlines()[2:] if ':' in line}

def digest(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def write(name,value):
    (OUT/name).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')

def inventory(root):
    return {p.relative_to(root).as_posix():{'sha256':digest(p),'mode':p.stat().st_mode&0o777}
        for p in sorted(root.rglob('*')) if p.is_file()}

def group():
    return Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')

def resources():
    p=group()
    return {'cgroup':str(p.relative_to('/sys/fs/cgroup')),'memory_max_bytes':int((p/'memory.max').read_text()),
        'swap_max_bytes':int((p/'memory.swap.max').read_text()),'peak_bytes':int((p/'memory.peak').read_text()),
        'events':{k:int(v) for k,v in (s.split() for s in (p/'memory.events').read_text().splitlines())}}

def commands():
    # Extract only literal top-level calls before the first conditional.
    # No eval, no executing local-small.py, no synthetic command substitute.
    result=[]
    for node in ast.parse((SOURCE/'scripts/local-small.py').read_text()).body:
        if isinstance(node,ast.If):break
        if not isinstance(node,ast.Expr) or not isinstance(node.value,ast.Call):continue
        call=node.value
        if not isinstance(call.func,ast.Attribute) or not isinstance(call.func.value,ast.Name):raise ValueError('unknown top-level call')
        if (call.func.value.id,call.func.attr)!=('subprocess','run'):raise ValueError('unexpected top-level call')
        assert len(call.args)==1 and isinstance(call.args[0],ast.List)
        assert {k.arg for k in call.keywords}=={'cwd','check'}
        fields={k.arg:k.value for k in call.keywords}
        assert isinstance(fields['cwd'],ast.Name) and fields['cwd'].id=='root'
        assert isinstance(fields['check'],ast.Constant) and fields['check'].value is True
        argv=[]
        for arg in call.args[0].elts:
            if isinstance(arg,ast.Constant) and isinstance(arg.value,str):argv.append(arg.value)
            elif isinstance(arg,ast.Attribute) and isinstance(arg.value,ast.Name) and (arg.value.id,arg.attr)==('sys','executable'):argv.append(sys.executable)
            else:raise ValueError('nonliteral command argument')
        assert argv[0] in ('node','bun',sys.executable)
        result.append(argv)
    assert len(result)==12,('prefix changed',result)
    return result

def worker():
    os.chdir(SOURCE)
    cap=resources();assert cap['memory_max_bytes']==3699998720 and cap['swap_max_bytes']==0
    assert shutil.which('bend') is None,'No Bend is needed or permitted in this profiling VM'
    inputs=json.loads((OUT/'INPUTS.json').read_text());assert inventory(SOURCE)==inputs['files']
    tools={name:{'path':str(Path(shutil.which(name)).resolve()),'sha256':digest(shutil.which(name))}
        for name in ('bun','node','git','/usr/bin/time')}
    assert tools['bun']['sha256']==BUN_SHA
    assert tools['node']['sha256']==NODE_SHA
    for name in ('bun','node'):
        tools[name]['version']=subprocess.check_output([tools[name]['path'],'--version'],text=True,timeout=5).strip()
    lock=os.open('/tmp/bp-bend.lock',os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o666)
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    probe=os.open('/tmp/bp-bend.lock',os.O_RDWR|os.O_NOFOLLOW)
    try:
        try:fcntl.flock(probe,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:pass
        else:raise RuntimeError('native lock not held')
    finally:os.close(probe)
    os.set_inheritable(lock,True)
    argv_list=commands()
    selected=list(enumerate(argv_list))[7:]
    assert [argv[1:] for _,argv in selected]==[
        ['test/tools/store_promote_test.cjs'],['test/tools/hotboot_wire_test.cjs'],
        ['test/tools/kicad_library_header_test.py'],['scripts/kistack.py','verify'],['test/tools/kistack_test.py']]
    env_overrides={'PYTHONDONTWRITEBYTECODE':'1','BACKPLANE_HEAVY_LOCK_FD':str(lock),'BEND_NO_TELEMETRY':'1',
        'BACKPLANE_TELEMETRY':'off','BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_NO_UPDATE':'1'}
    write('STARTED.json',{'source_revision':REV,'source_tree':TREE,'harness_sha256':digest(SELF),'resources':cap,
        'commands':argv_list,'selected_indices':[i for i,_ in selected],
        'environment_overrides':env_overrides,'local_small_sha256':digest(SOURCE/'scripts/local-small.py'),'tools':tools,
        'python_version':sys.version,'python_sha256':digest(sys.executable),'actual_native_flock':True,
        'pid':os.getpid(),'pid_start':Path('/proc/self/stat').read_text().split()[21],
        'scope':'Five previously unexecuted real guards only; no complete small phase or compiler',
        'limits':'One cold sequential observation per command, not a comparison. Inherited environment differs from production and is not captured; explicit overrides are recorded. Profiles continue after failure, unlike production.'})
    records=[]
    started=time.monotonic()
    env={**os.environ,**env_overrides}
    try:
        for i,argv in selected:
            prefix=OUT/('guard-'+str(i).zfill(2));tm=prefix.with_suffix('.time.json')
            net_before=network_bytes();before=resources();cmd_started=time.monotonic();timed_out=False
            with prefix.with_suffix('.stdout').open('wb') as out,prefix.with_suffix('.stderr').open('wb') as err:
                p=subprocess.Popen(['/usr/bin/time','-f','{"wall_seconds":%e,"peak_rss_kib":%M,"user_seconds":%U,"system_seconds":%S}',
                    '-o',str(tm),*argv],cwd=SOURCE,env=env,pass_fds=(lock,),stdout=out,stderr=err,start_new_session=True)
                try:exit_code=p.wait(timeout=90)
                except subprocess.TimeoutExpired:
                    timed_out=True
                    try:os.killpg(p.pid,signal.SIGTERM)
                    except ProcessLookupError:pass
                    try:p.wait(timeout=2)
                    except subprocess.TimeoutExpired:pass
                    # Kill any descendants left in this owned group even if its leader exited.
                    try:os.killpg(p.pid,signal.SIGKILL)
                    except ProcessLookupError:pass
                    p.wait(timeout=5);exit_code=124
            # GNU time can prepend a nonzero-exit description before its JSON.
            lines=tm.read_text().splitlines() if tm.exists() else []
            data=next((s for s in reversed(lines) if s.startswith('{')),None)
            metric=json.loads(data) if data else {'wall_seconds':time.monotonic()-cmd_started,'peak_rss_kib':None,
                'user_seconds':None,'system_seconds':None}
            net_after=network_bytes()
            record={'index':i,'command':argv,'exit':exit_code,'timed_out':timed_out,**metric,
                'elapsed_seconds':time.monotonic()-cmd_started,'resources_before':before,'resources_after':resources(),
                'network_interface_byte_deltas':{name:{key:net_after[name][key]-net_before[name][key]
                    for key in ('received','sent')} for name in net_before.keys()&net_after.keys()},
                'network_limit':'VM-wide counters include unrelated traffic; they cannot attribute a request to this command.',
                'stdout_sha256':digest(prefix.with_suffix('.stdout')),'stderr_sha256':digest(prefix.with_suffix('.stderr'))}
            records.append(record);write('COMMANDS.json',records)
            print(json.dumps(record),flush=True)
    finally:
        after_inventory=inventory(SOURCE)
        write('FINAL-IDENTITY.json',{'source_unchanged':after_inventory==inputs['files'],
            'source_diff':{name:{'before':inputs['files'].get(name),'after':after_inventory.get(name)}
                for name in inputs['files'].keys()|after_inventory.keys() if inputs['files'].get(name)!=after_inventory.get(name)},
            'harness_unchanged':digest(SELF)==inputs['harness_sha256'],
            'tools_unchanged':all(digest(v['path'])==v['sha256'] for v in tools.values()),'resources':resources()})
    final=json.loads((OUT/'FINAL-IDENTITY.json').read_text())
    intact=all(final[k] for k in ('source_unchanged','harness_unchanged','tools_unchanged'))
    bounded=final['resources']['peak_bytes']<=final['resources']['memory_max_bytes'] and all(
        final['resources']['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
    write('MEASUREMENT.json',{'kind':'profiling real guard commands, not a CI gate','source_revision':REV,'source_tree':TREE,
        'commands_completed':len(records),'guards_passed':all(r['exit']==0 for r in records),
        'failed_commands':[r for r in records if r['exit']!=0],
        'wall_seconds':time.monotonic()-started,'command_wall_sum':sum(r['wall_seconds'] for r in records),
        'largest_observed_command_peak_rss_kib':max((r['peak_rss_kib'] for r in records if r['peak_rss_kib'] is not None),default=None),
        'identity_intact':intact,'source_diff':final['source_diff'],'resources':final['resources'],
        'resources_intact':bounded,'qualification':False,
        'limits':'Frozen original five remaining commands only; no repeats of first seven guards. One sequential observation per command, no variance or speedup claim. Final Bend, deps, desktop and full CI not executed. Failures stay failures.'})
    return 0 if intact and bounded and len(records)==5 else 1

def controller():
    assert resources()['memory_max_bytes']==536870912 and resources()['swap_max_bytes']==0
    assert not OUT.exists(),'already started; never retry in the same VM'
    OUT.mkdir(parents=True)
    repo=OUT/'fetch.git'
    def git(*args):
        return subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=0','-c','pack.threads=1','-c','pack.windowMemory=32m',
            '-C',str(repo),*args],stderr=subprocess.PIPE,timeout=120)
    subprocess.run(['git','init','--bare',str(repo)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,check=True)
    git('remote','add','origin','https://github.com/BackplaneWorks/Backplane.git')
    git('config','remote.origin.promisor','true');git('config','remote.origin.partialclonefilter','blob:none')
    git('fetch','--filter=blob:none','--no-tags','origin',REV)
    assert git('rev-parse','FETCH_HEAD').decode().strip()==REV and git('rev-parse',REV+'^{tree}').decode().strip()==TREE
    archive=OUT/'source.tar'
    with archive.open('wb') as f:subprocess.run(['git','-C',str(repo),'archive',REV],stdout=f,stderr=subprocess.PIPE,check=True,timeout=120)
    SOURCE.mkdir()
    with tarfile.open(archive) as tar:tar.extractall(SOURCE,filter='data')
    archive.unlink();shutil.rmtree(repo)
    write('INPUTS.json',{'revision':REV,'tree':TREE,'files':inventory(SOURCE),'harness_sha256':digest(SELF)})
    unit='bp-remaining-profile-'+str(os.getpid())+'.scope'
    timed_out=False
    try:
        with (OUT/'worker.log').open('w') as log:
            r=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,'-p','MemoryMax=3700000000','-p','MemorySwapMax=0',
                '--','sh','scripts/heavy-boat.sh',sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,
                env={**os.environ,'BP_SMALL_OUT':str(OUT)},stdout=log,stderr=subprocess.STDOUT,timeout=520)
    except subprocess.TimeoutExpired:
        timed_out=True
        subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20)
        r=subprocess.CompletedProcess([],124)
    measurement=json.loads((OUT/'MEASUREMENT.json').read_text()) if (OUT/'MEASUREMENT.json').exists() else None
    write('SUMMARY.json',{'kind':'real non-Bend guard profiling, not CI qualification','worker_exit':r.returncode,
        'timed_out':timed_out,'measurement':measurement,'controller':resources(),'qualification':False,
        'combined_cap_bytes':4236869632,'source_revision':REV,'source_tree':TREE})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE)
    return 0 if r.returncode==0 and measurement is not None else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    raise SystemExit(worker() if sys.argv[1:]==['worker'] else controller())
