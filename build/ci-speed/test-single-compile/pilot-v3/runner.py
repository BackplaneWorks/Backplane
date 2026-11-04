"""Corrected matched stock Bend IO + module-driver pilot. Boat only."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import statistics
import subprocess
import sys
import tarfile
import threading
import time
import urllib.request

REV='1e34e40673417e8388ce23c15795e5de9ecd214a'
TREE='575db83698781f0c3f54ab32d772afde62308494'
TEST='test/clipath_test.bend'
TEST_SHA='16e9b5bb59afa3f649e03979e9d760cf07dcbbb8c65b5773355b987319e2bf21'
PIN='2.0.36 02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba'
BUN_SHA='795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
SELF=Path(__file__).resolve()
OUT=Path(os.environ.get('BP_TEST_ONCE_OUT',str(Path.cwd()/'build/test-single-compile')))
SOURCE=OUT/'source'

def digest(p):
    h=hashlib.sha256()
    with Path(p).open('rb') as f:
        for chunk in iter(lambda:f.read(65536),b''):h.update(chunk)
    return h.hexdigest()

def require_bytes(path,expected):
    if digest(path)!=expected:
        raise ValueError('artifact byte identity mismatch: '+str(path))

def group():
    return Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')

def resources(at):
    return {'cgroup':str(at.relative_to('/sys/fs/cgroup')),
        'memory_max_bytes':int((at/'memory.max').read_text()),'swap_max_bytes':int((at/'memory.swap.max').read_text()),
        'peak_bytes':int((at/'memory.peak').read_text()),
        'events':{k:int(v) for k,v in (s.split() for s in (at/'memory.events').read_text().splitlines())}}

def write(name,value):
    (OUT/name).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')

def inventory(root):
    return {p.relative_to(root).as_posix():{'sha256':digest(p),'mode':p.stat().st_mode&0o777}
        for p in sorted(root.rglob('*')) if p.is_file()}

def worker():
    os.chdir(SOURCE)
    before=resources(group())
    assert before['memory_max_bytes']==3699998720 and before['swap_max_bytes']==0
    inputs=json.loads((OUT/'INPUTS.json').read_text())
    assert inventory(SOURCE)==inputs['files'] and digest(SOURCE/TEST)==TEST_SHA
    lock=os.open('/tmp/bp-bend.lock',os.O_CREAT|os.O_RDWR|os.O_NOFOLLOW,0o666)
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    probe=os.open('/tmp/bp-bend.lock',os.O_RDWR|os.O_NOFOLLOW)
    try:
        try:fcntl.flock(probe,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:pass
        else:raise RuntimeError('native bend flock not held')
    finally:os.close(probe)
    os.set_inheritable(lock,True)
    bend=Path(shutil.which('bend')).resolve()
    bun=Path(shutil.which('bun')).resolve()
    identity=json.loads((OUT/'TOOL-IDENTITY.json').read_text())
    assert (Path.home()/'.bend/.pinned').read_text().strip()==PIN
    assert digest(bend)==identity['binary_sha256'] and digest(bun)==BUN_SHA
    library=Path.home()/'.bend/bend2'
    assert inventory(library)==identity['library']
    env={**os.environ,'BEND_NO_TELEMETRY':'1','BACKPLANE_HEAVY_LOCK_FD':str(lock),
        'BUN_JSC_forceRAMSize':'4294967296','BUN_JSC_maxPerThreadStackUsage':'536870912',
        'BP_SINGLE_COMPILE_FIXTURE':'public fixture 日本'}
    write('STARTED.json',{'source_revision':REV,'source_tree':TREE,'resources':before,
        'bend_sha256':digest(bend),'bun_sha256':digest(bun),'actual_native_bend_flock':True,
        'pid':os.getpid(),'pid_start':Path('/proc/self/stat').read_text().split()[21],
        'guard':'merged heavy-boat.sh + compiler-guard.py shim; isolated capped VM',
        'stack_limits':list(resource.getrlimit(resource.RLIMIT_STACK)),
        'environment':{k:env[k] for k in ('BEND_NO_TELEMETRY','BUN_JSC_forceRAMSize','BUN_JSC_maxPerThreadStackUsage')}})
    launcher=OUT/'run-io.mjs'
    launcher.write_text("import {pathToFileURL} from 'node:url';\nimport {resolve} from 'node:path';\n"
        "const [artifact, source, ...args] = process.argv.slice(2);\n"
        "if (!artifact || !source) throw Error('artifact and original source required');\n"
        "process.argv = [process.argv[0], source, '--', ...args];\n"
        "await import(pathToFileURL(resolve(artifact)).href);\n")
    records=[]
    def run(label,args,input_bytes=None):
        tm=OUT/(label+'.time');log=OUT/(label+'.log')
        started=time.monotonic()
        stdout=OUT/(label+'.stdout');stderr=OUT/(label+'.stderr')
        with stdout.open('wb') as out,stderr.open('wb') as err:
            r=subprocess.run(['/usr/bin/time','-f','%M','-o',str(tm),*map(str,args)],env=env,
                pass_fds=(lock,),stdout=out,stderr=err,input=input_bytes,timeout=45)
        log.write_bytes(stdout.read_bytes()+stderr.read_bytes())
        numbers=[int(line) for line in tm.read_text().splitlines() if line.isdecimal()]
        record={'label':label,'exit':r.returncode,'seconds':time.monotonic()-started,
            'peak_rss_kib':numbers[-1] if numbers else None,'log_sha256':digest(log),
            'stdout_sha256':digest(stdout),'stderr_sha256':digest(stderr)}
        records.append(record);write('COMMANDS.json',records)
        return record,log.read_bytes()
    def compile_command(*args):
        return [sys.executable,'-IB','scripts/compiler-guard.py','shim','bend',str(bend),'bend','--',*args]
    test_script=(SOURCE/'scripts/test.sh').read_text()
    verdict=test_script.split('status=0\ni=0\nfor t in $tests; do\n',1)[1].split('# every test passed, unsharded:',1)[0]
    verdict='status=0\ni=0\nfor t in $tests; do\n'+verdict
    verdict_script=OUT/'production-verdict.sh'
    verdict_script.write_text('set -eu\nwork=$1\ntests=fixture\ncost() { :; }\n'+verdict+'\nexit "$status"\n')
    write('PRODUCTION-VERDICT.json',{'test_sh_sha256':digest(SOURCE/'scripts/test.sh'),
        'extracted_replay_sha256':hashlib.sha256(verdict.encode()).hexdigest(),
        'note':'Exact production replay block, cost display stubbed; does not execute test.sh setup or mark passes.'})
    verdict_work=OUT/'verdict';verdict_work.mkdir()
    def strict(r,output):
        (verdict_work/'0.rc').write_text(str(r['exit'])+'\n');(verdict_work/'0.out').write_bytes(output)
        v=subprocess.run(['sh',str(verdict_script),str(verdict_work)],capture_output=True,timeout=5)
        return v.returncode==0
    def path_run(label,source,mode,args=(),driver=None,input_bytes=None):
        module=OUT/(label+'.mjs');executable=OUT/(label+'.js')
        results=[];ios=b'';drivers=b''
        if mode=='baseline':
            r,ios=run(label+'-io',compile_command(str(source),*(['--',*args] if args else [])),input_bytes);results.append(r)
            emit,log=run(label+'-emit',compile_command(str(source),'-o',str(module)));results.append(emit)
        else:
            emit,log=run(label+'-emit',compile_command(str(source),'-o',str(module),'-o',str(executable)));results.append(emit)
            assert emit['exit']==0,(label,'emit failed')
            r,ios=run(label+'-io',[bun,launcher,executable,source,*args],input_bytes);results.append(r)
        assert emit['exit']==0 and module.is_file(),(label,'missing library')
        if driver is not None:
            r,drivers=run(label+'-driver',[bun,driver,module]);results.append(r)
        return {'seconds':sum(r['seconds'] for r in results),'peak_rss_kib':max(r['peak_rss_kib'] or 0 for r in results),
            'io':ios,'driver':drivers,'results':results,'module_sha256':digest(module),
            'executable_sha256':digest(executable) if executable.is_file() else None}
    fixtures=OUT/'fixtures';fixtures.mkdir()
    argv=fixtures/'argv.bend'
    argv.write_text('import Base\ndef Args.show(xs: List<String>) -> String:\n  match xs:\n'
        '    case Nil{}: ""\n    case h <> t: h ++ "|" ++ Args.show(t)\n'
        'def main() -> IO(Unit):\n  do IO<Unit>:\n    xs : List<String> <- IO.args()\n    IO.print("ok args:" ++ Args.show(xs))\n')
    for name,body in [('false-line','    IO.print("ok preliminary")\n    IO.print("FAIL retained assertion")\n'),
        ('die','    IO.print("ok preliminary")\n    IO.die(Unit, 7, "retained failure")\n')]:
        (fixtures/(name+'.bend')).write_text('import Base\ndef main() -> IO(Unit):\n  do IO<Unit>:\n'+body)
    bad=fixtures/'bad.bend';bad.write_text('import Base\ndef bad() -> U32:\n  "wrong"\n')
    (OUT/'context.txt').write_text('cwd-relative public input 日本')
    (fixtures/'context.js').write_text('function probe_context() {\n'
        ' const fs = require("node:fs");\n'
        ' return "ok context:" + JSON.stringify({cwd:process.cwd(), env:process.env.BP_SINGLE_COMPILE_FIXTURE, '
        'file:fs.readFileSync("../context.txt","utf8"), stdin:fs.readFileSync(0,"utf8")}) + "\\n" + "x".repeat(1048576);\n'
        '}\nio_eff(CID(Probe.context), probe_context);\n')
    (fixtures/'context.bend').write_text('import Base\ndef Probe.context() -> IO(String):\n  import "./context.js"\n'
        'def main() -> IO(Unit):\n  do IO<Unit>:\n    text : String <- Probe.context()\n    IO.print(text)\n')
    (fixtures/'depth.bend').write_text('import Base\ndef depth(+n: U32) -> U32:\n  match U32.is_eq(n, 0):\n'
        '    case True{}: 0\n    case False{}: U32.add(depth(U32.sub(n, 1)), 1)\n'
        'def main() -> IO(Unit):\n  IO.print("ok depth:" ++ U32.show(depth(50000)))\n')
    driver=fixtures/'fail-driver.mjs'
    driver.write_text("import {pathToFileURL} from 'node:url';\nawait import(pathToFileURL(process.argv[2]).href);\nconsole.log('ok preliminary'); throw Error('retained driver failure');\n")
    fixture_inputs=inventory(fixtures)
    write('FIXTURE-INPUTS.json',fixture_inputs)
    # Validate all generated source before any matched timing observation.
    for p in sorted(fixtures.glob('*.bend')):
        r,log=run('preflight-'+p.stem,compile_command(str(p),'-o',str(OUT/('preflight-'+p.stem+'.mjs'))))
        assert (r['exit']==0)==(p!=bad),('invalid fixture',p.name)
    comparisons=[]
    # j986353 lacked its after-run identity fence. Keep it historical; this
    # independently reviewed correction gets one complete hash-bound receipt.
    for trial in range(3):
        modes=('baseline','candidate') if trial%2==0 else ('candidate','baseline')
        pair={mode:path_run(f'clipath-{trial}-{mode}',TEST,mode,driver='test/tools/clipath.mjs') for mode in modes}
        a,b=pair['baseline'],pair['candidate']
        for mode,r in pair.items():
            by_label={entry['label'].rsplit('-',1)[-1]:entry for entry in r['results']}
            assert strict(by_label['io'],r['io']) and strict(by_label['driver'],r['driver']),(trial,mode,'assertions missing or failed')
            assert all(entry['exit']==0 for entry in r['results'])
        assert a['module_sha256']==b['module_sha256'],'library bytes differ'
        require_bytes(OUT/f'clipath-{trial}-candidate.mjs',a['module_sha256'])
        assert a['io']==b['io'] and a['driver']==b['driver'],'assertion output differs'
        for kind in ('io','driver'):
            ra=next(r for r in a['results'] if r['label'].endswith('-'+kind))
            rb=next(r for r in b['results'] if r['label'].endswith('-'+kind))
            assert (ra['stdout_sha256'],ra['stderr_sha256'])==(rb['stdout_sha256'],rb['stderr_sha256']),'output channel differs'
        comparisons.append({mode:{k:v for k,v in r.items() if k not in ('io','driver')} for mode,r in pair.items()})
    standalone=OUT/'clipath-standalone.js'
    r,log=run('standalone-executable',compile_command(TEST,'-o',str(standalone)))
    assert r['exit']==0
    for p in comparisons:require_bytes(standalone,p['candidate']['executable_sha256'])
    tampered=OUT/'tampered.js';tampered.write_bytes(standalone.read_bytes()+b'\n// altered\n')
    try:require_bytes(tampered,comparisons[0]['candidate']['executable_sha256'])
    except ValueError:pass
    else:raise AssertionError('actual byte fence accepted tampering')
    special=['space value','日本','--threads','99','--gpu','x','--bend-help','--','']
    argv_pair={mode:path_run('argv-'+mode,argv,mode,special) for mode in ('baseline','candidate')}
    expected=('ok args:'+str(argv)+'|'+'|'.join(special)+'|\n').encode()
    assert all(p['io']==expected and all(r['exit']==0 for r in p['results']) for p in argv_pair.values()),'IO.args identity differs'
    assert argv_pair['baseline']['module_sha256']==argv_pair['candidate']['module_sha256']
    controls=['original argv incl spaces unicode flag-like tokens and empty argument','exact argument-control library bytes',
        'exact standalone stock executable bytes','executable byte corruption rejected','exact IO and driver stdout+stderr','actual production verdict replay']
    for name,stdin in [('context',('public stdin 日本\n'*1000).encode()),('depth',None)]:
        pair={mode:path_run(name+'-'+mode,fixtures/(name+'.bend'),mode,input_bytes=stdin) for mode in ('baseline','candidate')}
        a,b=pair['baseline'],pair['candidate']
        assert a['module_sha256']==b['module_sha256'] and a['io']==b['io'],name+' parity failed'
        for value in pair.values():
            io=next(r for r in value['results'] if r['label'].endswith('-io'))
            assert strict(io,value['io']) and all(r['exit']==0 for r in value['results'])
        ra=next(r for r in a['results'] if r['label'].endswith('-io'));rb=next(r for r in b['results'] if r['label'].endswith('-io'))
        assert (ra['stdout_sha256'],ra['stderr_sha256'])==(rb['stdout_sha256'],rb['stderr_sha256'])
        if name=='context':
            assert len(a['io'])>1048576 and b'cwd-relative public input' in a['io'] and b'public fixture' in a['io'] and b'public stdin' in a['io']
        else:assert a['io']==b'ok depth:50000\n'
        controls.append(name+' exact bytes, channels and exit; '+('stdin, env, cwd-relative input and 1MiB flush' if name=='context' else '50000 non-tail recursive calls, not a stack-boundary qualification'))
    for name,body,expected_exit in [('false-line','    IO.print("ok preliminary")\n    IO.print("FAIL retained assertion")\n',0),
        ('die','    IO.print("ok preliminary")\n    IO.die(Unit, 7, "retained failure")\n',7)]:
        fixture=fixtures/(name+'.bend')
        outputs=[]
        for mode in ('baseline','candidate'):
            r=path_run(name+'-'+mode,fixture,mode)
            io=next(s for s in r['results'] if s['label'].endswith('-io'))
            assert io['exit']==expected_exit and not strict(io,r['io']),(name,mode,'false green')
            outputs.append(r['io'])
        assert outputs[0]==outputs[1],name+' runtime error output differs'
        controls.append(name+' rejected by unchanged strict verdict')
    r,log=run('failing-driver',[bun,driver,OUT/'argv-candidate.mjs'])
    assert r['exit']!=0 and not strict(r,log);controls.append('driver exception rejected')
    for mode in ('baseline','candidate'):
        outs=['-o',str(OUT/(mode+'-bad.mjs'))]
        if mode=='candidate':outs+=['-o',str(OUT/(mode+'-bad.js'))]
        r,log=run('bad-'+mode,compile_command(str(bad),*outs))
        assert r['exit']!=0 and not (OUT/(mode+'-bad.mjs')).exists() and not (OUT/(mode+'-bad.js')).exists(),'type error accepted or output created'
    controls.append('actual type error rejected before emit in both paths')
    unchanged=(inventory(SOURCE)==inputs['files'] and digest(bend)==identity['binary_sha256']
        and digest(bun)==BUN_SHA and inventory(library)==identity['library']
        and digest(SELF)==inputs['harness_sha256'] and inventory(fixtures)==fixture_inputs)
    after=resources(group())
    accepted=unchanged and after['peak_bytes']<=after['memory_max_bytes'] and all(after['events'].get(k)==0 for k in ('oom','oom_kill','oom_group_kill'))
    write('EXECUTION.json',{'source_revision':REV,'source_tree':TREE,'test':TEST,'test_sha256':TEST_SHA,
        'comparisons':comparisons,'prior_unqualified_job':'j20261104043509-986353',
        'median_seconds':{mode:statistics.median(p[mode]['seconds'] for p in comparisons) for mode in ('baseline','candidate')},
        'controls':controls,'resources_before':before,'resources_after':after,'source_tool_unchanged':unchanged,
        'actual_native_bend_flock':True,'qualified':accepted,'scope':'clipath IO+driver only; not full suite, proof, workflow or release',
        'unqualified_contexts':['other eligible tests','raw process.execPath/argv metadata','stack boundary beyond 50000 calls']})
    return 0 if accepted else 1

def controller():
    assert resources(group())['memory_max_bytes']==536870912 and resources(group())['swap_max_bytes']==0
    assert not OUT.exists(),'attempt exists; inspect, never rerun'
    OUT.mkdir(parents=True)
    write('CONTROLLER-STARTED.json',{'resources':resources(group()),'harness_sha256':digest(SELF),'source_revision':REV})
    write('HOST-TOOLS.json',{'python':sys.version,'python_sha256':digest(sys.executable),
        'tools':{tool:{'path':shutil.which(tool),'sha256':digest(shutil.which(tool)),
            'version':subprocess.check_output([tool,'--version'],stderr=subprocess.STDOUT,timeout=10).decode().splitlines()[0]}
            for tool in ('git','systemd-run','/usr/bin/time')}})
    repo=OUT/'fetch.git'
    def git(*args):
        return subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=0','-c','pack.threads=1',
            '-c','pack.windowMemory=32m','-C',str(repo),*args],stderr=subprocess.PIPE,timeout=120)
    subprocess.run(['git','init','--bare',str(repo)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
    git('remote','add','origin','https://github.com/BackplaneWorks/Backplane.git')
    git('config','remote.origin.promisor','true');git('config','remote.origin.partialclonefilter','blob:none')
    git('fetch','--filter=blob:none','--no-tags','origin',REV)
    assert git('rev-parse','FETCH_HEAD').decode().strip()==REV and git('rev-parse',REV+'^{tree}').decode().strip()==TREE
    archive=OUT/'source.tar'
    with archive.open('wb') as f:
        subprocess.run(['git','-c','gc.auto=0','-C',str(repo),'archive',REV,'src','test','scripts','LAWS.bend','PROOF.bend','.github/actions/setup-bend'],
            stdout=f,stderr=subprocess.PIPE,check=True,timeout=120)
    SOURCE.mkdir()
    with tarfile.open(archive) as tar:tar.extractall(SOURCE,filter='data')
    archive.unlink();shutil.rmtree(repo)
    assert digest(SOURCE/TEST)==TEST_SHA
    archive_sha=PIN.split()[1]
    candidates=sorted(Path('/tmp').glob('tmp.*/bend-2.0.36-linux-x64.tar.gz'))
    tool_archive=next((p for p in candidates if p.is_file() and digest(p)==archive_sha),None)
    if tool_archive is None:
        tool_archive=OUT/'official-bend.tar.gz'
        with urllib.request.urlopen('https://github.com/bendlang/bend/releases/download/v2.0.36/bend-2.0.36-linux-x64.tar.gz',timeout=60) as src,tool_archive.open('wb') as dst:shutil.copyfileobj(src,dst)
    assert digest(tool_archive)==archive_sha
    with tarfile.open(tool_archive) as tar:
        binary_sha=hashlib.sha256(tar.extractfile('bend/bin/bend').read()).hexdigest()
        library={}
        for m in tar.getmembers():
            if m.isfile() and m.name.startswith('bend/bend2/'):
                with tar.extractfile(m) as f:sha=hashlib.file_digest(f,'sha256').hexdigest()
                library[m.name[len('bend/bend2/'):]]={'sha256':sha,'mode':m.mode&0o777}
    assert library
    write('TOOL-IDENTITY.json',{'archive_sha256':archive_sha,'binary_sha256':binary_sha,'library':library,'bun_sha256':BUN_SHA})
    if tool_archive.parent==OUT:tool_archive.unlink()
    write('INPUTS.json',{'source_revision':REV,'source_tree':TREE,'files':inventory(SOURCE),'harness_sha256':digest(SELF)})
    # Inventory only: count actual eligible tests, never run the wider suite.
    eligible=[]
    for p in sorted((SOURCE/'test').glob('*_test.bend')):
        driver=SOURCE/'test/tools'/(p.name.removesuffix('_test.bend')+'.mjs')
        py=SOURCE/'test/tools'/(p.stem+'.py')
        if driver.is_file() and '\ndef main(' in '\n'+p.read_text() and not (py.is_file() and p.relative_to(SOURCE).as_posix() in py.read_text()):
            eligible.append(p.relative_to(SOURCE).as_posix())
    write('ELIGIBLE.json',{'count':len(eligible),'tests':eligible,'note':'Current runner main+driver branch metadata only; pilot covers clipath only.'})
    unit='bp-test-once-'+str(os.getpid())+'.scope'
    stop=threading.Event()
    def sample():
        with (OUT/'memory.jsonl').open('w') as f:
            while not stop.wait(0.1):
                try:
                    info=json.loads((OUT/'STARTED.json').read_text())
                    state=resources(Path('/sys/fs/cgroup')/info['resources']['cgroup'])
                    f.write(json.dumps(state)+'\n');f.flush()
                except (OSError,ValueError,KeyError):pass
    thread=threading.Thread(target=sample,daemon=True);thread.start()
    timed_out=False
    try:
        with (OUT/'worker.log').open('w') as log:
            result=subprocess.run(['systemd-run','--user','--scope','--quiet','--unit='+unit,
                '-p','MemoryMax=3700000000','-p','MemorySwapMax=0','--','sh','scripts/heavy-boat.sh',
                sys.executable,'-IB',str(SELF),'worker'],cwd=SOURCE,env={**os.environ,'BP_TEST_ONCE_OUT':str(OUT)},
                stdout=log,stderr=subprocess.STDOUT,timeout=240)
    except subprocess.TimeoutExpired:
        timed_out=True
        subprocess.run(['systemctl','--user','stop',unit],capture_output=True,timeout=20)
        result=subprocess.CompletedProcess([],124)
    finally:stop.set();thread.join(2)
    execution=json.loads((OUT/'EXECUTION.json').read_text()) if (OUT/'EXECUTION.json').exists() else None
    write('SUMMARY.json',{'kind':'matched stock single-compile IO+module-driver pilot','source_revision':REV,'source_tree':TREE,
        'worker_unit_exit':result.returncode,'timed_out':timed_out,'execution':execution,'controller':resources(group()),
        'combined_cap_bytes':4236869632,'qualified':execution is not None and execution['qualified'] and result.returncode==0,
        'full_test_proof_build_release_qualified':False,'limits':'Only clipath exercised; missing execution receipt is inconclusive.'})
    print((OUT/'SUMMARY.json').read_text(),flush=True)
    shutil.rmtree(SOURCE)
    return 0 if execution is not None and execution['qualified'] and result.returncode==0 else 1

if __name__=='__main__':
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    if sys.argv[1:]==['worker']:
        try:raise SystemExit(worker())
        finally:
            # Preserve the final identity fence even when a control raises.
            # An OOM/SIGKILL can still omit it and stays inconclusive.
            inputs=json.loads((OUT/'INPUTS.json').read_text())
            identity=json.loads((OUT/'TOOL-IDENTITY.json').read_text())
            write('FINAL-IDENTITY.json',{'source_unchanged':inventory(SOURCE)==inputs['files'],
                'harness_unchanged':digest(SELF)==inputs['harness_sha256'],
                'bend_unchanged':digest(Path(shutil.which('bend')).resolve())==identity['binary_sha256'],
                'bun_unchanged':digest(Path(shutil.which('bun')).resolve())==BUN_SHA,
                'library_unchanged':inventory(Path.home()/'.bend/bend2')==identity['library'],
                'resources':resources(group())})
    else:raise SystemExit(controller())
