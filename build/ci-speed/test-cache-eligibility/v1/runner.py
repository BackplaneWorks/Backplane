"""Read-only eligibility inventory. Never invoke Bend, tests, or seed passes."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import time

REV='29c5bc13f3deeba992332afd5494806c376ed860'
TREE='fc1d1df12a9fc8c1f751c3701344a24255d4547a'
SELF=Path(__file__).resolve()
OUT=Path.cwd()/'build/deterministic-test-inventory'
SOURCE=OUT/'source'

def digest(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def inventory(folder):
    return {p.relative_to(folder).as_posix():[digest(p),p.stat().st_mode&0o777]
        for p in sorted(folder.rglob('*')) if p.is_file()}

def write(name,value):
    (OUT/name).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')

def resources():
    p=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
    return {'memory_max_bytes':int((p/'memory.max').read_text()),'swap_max_bytes':int((p/'memory.swap.max').read_text()),
        'peak_bytes':int((p/'memory.peak').read_text()),'events':{k:int(v) for k,v in (line.split() for line in (p/'memory.events').read_text().splitlines())}}

assert resources()['memory_max_bytes']==536870912 and resources()['swap_max_bytes']==0
assert os.environ.get('BEND_NO_TELEMETRY')=='1'
assert not OUT.exists(),'inspect existing attempt; do not overwrite/restart'
OUT.mkdir(parents=True)
repo=OUT/'fetch.git'
def git(*args):
    return subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=0','-c','pack.threads=1','-c','pack.windowMemory=32m',
        '-C',str(repo),*args],stderr=subprocess.PIPE,timeout=120)
subprocess.run(['git','init','--bare',str(repo)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
git('remote','add','origin','https://github.com/BackplaneWorks/Backplane.git')
git('config','remote.origin.promisor','true');git('config','remote.origin.partialclonefilter','blob:none')
git('fetch','--filter=blob:none','--no-tags','origin',REV)
assert git('rev-parse','FETCH_HEAD').decode().strip()==REV
assert git('rev-parse',REV+'^{tree}').decode().strip()==TREE
archive=OUT/'source.tar'
with archive.open('wb') as f:subprocess.run(['git','-C',str(repo),'archive',REV],stdout=f,stderr=subprocess.PIPE,check=True,timeout=120)
SOURCE.mkdir()
with tarfile.open(archive) as tar:tar.extractall(SOURCE,filter='data')
archive.unlink();shutil.rmtree(repo)
before=inventory(SOURCE);harness=digest(SELF)
write('INPUTS.json',{'revision':REV,'tree':TREE,'files':before,'harness_sha256':harness})
spec=importlib.util.spec_from_file_location('select_inventory',SOURCE/'scripts/ci-select.py')
selector=importlib.util.module_from_spec(spec);spec.loader.exec_module(selector)
binary=Path(shutil.which('bend')).resolve()
tool={'path':str(binary),'sha256':digest(binary),'library_sha256':selector.library_identity(binary.parent.parent/'bend2')}
base=(binary.parent.parent/'bend2/base.bend').read_text()
tool['base_sha256']=hashlib.sha256(base.encode()).hexdigest()
error=None
try:ordinary=selector.compiler_model(binary)
except (ValueError,OSError) as exc:ordinary=set();error=str(exc)

def deterministic_opaque(text):
    assert hashlib.sha256(text.encode()).hexdigest()==selector.BASE_SHA
    blocks={}
    matches=list(re.finditer(r'^(def|law|type) ([A-Za-z_][A-Za-z_0-9.]*)',text,re.M))
    for i,m in enumerate(matches):blocks.setdefault(m[2],[]).append(text[m.start():matches[i+1].start() if i+1<len(matches) else len(text)])
    allowed={'./effs/'+name+ext for name in ('print','print_err','write') for ext in ('.c','.js')}
    refs={};opaque=set()
    for name,bodies in blocks.items():
        body='\n'.join(bodies);refs[name]=selector.tokens(body)-{name}
        for line in body.splitlines():
            if not re.match(r'^\s*import\b',line):continue
            imported=selector.IMPORT.fullmatch(line)
            if not imported or imported[1].strip('"') not in allowed:opaque.add(name)
    while True:
        more={name for name,used in refs.items() if used&opaque}
        if more<=opaque:return opaque
        opaque|=more

began=time.monotonic()
strict=deterministic_opaque(base)
graph=selector.Graph(SOURCE,strict)
tests=selector.all_tests(SOURCE)
required=sorted({line.split('#',1)[0].strip() for line in (SOURCE/'scripts/fast-tests.txt').read_text().splitlines() if line.split('#',1)[0].strip()})
assert set(required)<=set(tests)
records=[]
for test in tests:
    try:
        closure,opaque=graph.closure(test)
        reasons=[]
        if error:reasons.append('unsupported tool: '+error)
        if opaque:reasons.append('opaque driver/import/Base operation/unsafe')
        operations={name:sorted(selector.tokens((SOURCE/name).read_text())&strict) for name in closure}
        operations={name:values for name,values in operations.items() if values}
        records.append({'test':test,'required':test in required,'eligible':not reasons,'reasons':reasons,
            'source_files':len(closure),'source_bytes':sum((SOURCE/name).stat().st_size for name in closure),
            'opaque_Base_operations':operations})
    except (ValueError,OSError,UnicodeError) as exc:
        records.append({'test':test,'required':test in required,'eligible':False,'reasons':[str(exc)]})
intact=inventory(SOURCE)==before and digest(SELF)==harness and digest(binary)==tool['sha256'] and selector.library_identity(binary.parent.parent/'bend2')==tool['library_sha256']
result={'kind':'deterministic standalone test-cache eligibility RESEARCH ONLY','source_revision':REV,'source_tree':TREE,
    'tool':tool,'tool_model_error':error,'selector_sha256':digest(SOURCE/'scripts/ci-select.py'),
    'classification_seconds':time.monotonic()-began,'tests':len(tests),'eligible':sum(r['eligible'] for r in records),
    'required':[r for r in records if r['required']],'records':records,'identity_intact':intact,'resources':resources(),
    'policy':'Conservative full-module closure; only console print/write native effects allowed. Clock/env/random/process/file/channel/spawn/custom effects/drivers opaque.',
    'limits':'Metadata design research only; eligibility classification is not a soundness proof or permission to skip. No test/compiler/proof execution or cache pass created.',
    'test_executions':0,'compiler_executions':0,'proof_executions':0,'pass_records_created':0,'gate_qualified':False}
write('RESULT.json',result)
print(json.dumps({k:v for k,v in result.items() if k not in ('records','required')},sort_keys=True),flush=True)
print(json.dumps({'required':result['required']},sort_keys=True),flush=True)
shutil.rmtree(SOURCE)
assert intact and result['resources']['peak_bytes']<=536870912 and all(result['resources']['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
