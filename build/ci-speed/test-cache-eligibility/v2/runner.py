"""Read-only eligibility inventory. Never invoke Bend, tests, or seed passes."""
import hashlib
from collections import Counter
import importlib.util
import json
import math
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
OUT=Path.cwd()/'build/deterministic-test-inventory-v2'
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

strict=deterministic_opaque(base)

def plain(text):
    # Preserve newlines: a quoted/commented main is not a runnable declaration.
    return selector.LITERALS.sub(lambda m:''.join('\n' if c=='\n' else ' ' for c in m[0]),text)

class Classifier:
    def __init__(self,root,tool_error=None):
        self.root=Path(root);self.tool_error=tool_error
        self.graph=selector.Graph(root,strict);self.modules={}

    def module(self,name):
        if name in self.modules:return self.modules[name]
        body=(self.root/name).read_text();code=plain(body);reasons=[]
        operations=sorted(selector.tokens(body)&strict)
        if operations:reasons.append({'code':'opaque_base_operation','file':name,'operations':operations})
        if re.search(r'@unsafe\b',code):reasons.append({'code':'unsafe','file':name})
        for line in body.splitlines():
            if not re.match(r'^\s*import\b',line):continue
            imported=selector.IMPORT.fullmatch(line)
            if not imported:reasons.append({'code':'unrecognized_import','file':name,'line':line});continue
            target,alias=imported.groups();target=target.strip('"')
            if target.startswith(('./','../')):
                if not target.endswith('.bend'):reasons.append({'code':'custom_effect','file':name,'import':target})
            elif target!='Base':reasons.append({'code':'named_import','file':name,'import':target})
            elif alias:reasons.append({'code':'base_alias','file':name,'alias':alias})
        mains=re.findall(r'^def main\s*\(\s*\)\s*->\s*IO\s*\(\s*Unit\s*\)\s*:',code,re.M)
        value={'reasons':reasons,'operations':operations,'bytes':len(body.encode()),'runnable_main_count':len(mains)}
        self.modules[name]=value;return value

    def classify(self,test):
        reasons=[]
        if self.tool_error:reasons.append({'code':'unsupported_tool','detail':self.tool_error})
        try:
            count=self.module(test)['runnable_main_count']
            if not count:reasons.append({'code':'missing_io_main','file':test})
            elif count!=1:reasons.append({'code':'ambiguous_io_main','file':test,'count':count})
        except (ValueError,OSError,UnicodeError) as exc:
            reasons.append({'code':'unreadable_entry','detail':str(exc)})
        stem=Path(test).stem;short=stem.removesuffix('_test')
        recognized={short+'.mjs',stem+'.py'}
        folder=self.root/'test/tools'
        matching=sorted(p for p in folder.iterdir() if p.name.startswith(short)) if folder.exists() else []
        for p in matching:
            reasons.append({'code':'opaque_driver' if p.name in recognized else 'unrecognized_driver_spelling',
                'file':p.relative_to(self.root).as_posix(),'directory':p.is_dir()})
        closure=set();operations={};source_bytes=0
        try:
            closure,opaque=self.graph.closure(test)
            for name in sorted(closure):
                info=self.module(name);reasons.extend(info['reasons']);source_bytes+=info['bytes']
                if info['operations']:operations[name]=info['operations']
            # Do not turn an unexplained opaque Graph result into an eligible one.
            if opaque and not any(r['code'] in ('opaque_base_operation','unsafe','custom_effect','named_import','base_alias',
                'unrecognized_import','opaque_driver','unrecognized_driver_spelling') for r in reasons):
                reasons.append({'code':'unexplained_graph_opacity'})
        except (ValueError,OSError,UnicodeError) as exc:
            reasons.append({'code':'invalid_dependency_graph','detail':str(exc)})
            # Keep already-readable entry diagnostics as well as the graph refusal.
            try:
                for reason in self.module(test)['reasons']:
                    if reason not in reasons:reasons.append(reason)
            except (ValueError,OSError,UnicodeError):pass
        return {'test':test,'eligible':not reasons,'reasons':reasons,'reason_codes':sorted({r['code'] for r in reasons}),
            'source_files':len(closure),'source_bytes':source_bytes,'opaque_Base_operations':operations}

def controls():
    result=[]
    excluded=['IO.get_env','IO.args','IO.now','IO.sleep','IO.random_u32','Process.run','IO.spawn','IO.thread_count',
        'Chan.new','Chan.send','Chan.recv','Chan.try_send','Chan.try_recv','Chan.close','IO.fork','IO.join','IO.within',
        'File.open','File.read','File.read_bytes','File.read_at','File.size','File.write','File.write_bytes','File.close','Socket.close']
    allowed=['IO.print','IO.print_err','IO.write']
    for name in excluded:
        assert name in strict,('opaque Base operation was lost',name)
        result.append({'case':'Base excludes '+name,'passed':True})
    for name in allowed:
        assert name not in strict,('console-only operation became opaque',name)
        result.append({'case':'Base allows '+name,'passed':True})
    fixtures=OUT/'classifier-controls';fixtures.mkdir()
    prefix='import Base\ndef main() -> IO(Unit):\n  IO.print("ok fixture")\n'
    index=0
    def case(label,text=prefix,extra=None,required_codes=(),eligible=False,tool_error=None):
        nonlocal index
        folder=fixtures/str(index);index+=1;(folder/'test/tools').mkdir(parents=True)
        name='test/sample_test.bend';(folder/name).write_text(text)
        for relative,body in (extra or {}).items():
            path=folder/relative;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(body)
        observed=Classifier(folder,tool_error).classify(name)
        assert observed['eligible'] is eligible,(label,observed)
        assert set(required_codes)<=set(observed['reason_codes']),(label,observed)
        result.append({'case':label,'passed':True,'expected_eligible':eligible,'observed':observed})
    case('print only must be eligible',eligible=True)
    case('comments and strings cannot invent effects',prefix+'# IO.now File.open @unsafe\ndef text() -> String:\n  "IO.get_env Chan.recv"\n',eligible=True)
    for name in excluded:
        # Scanner fixtures only, not compiled or presented as passing Bend tests.
        case('fixture excludes '+name,prefix+'\n# lexical dependency sentinel\ndef sentinel() -> Type:\n  '+name+'\n',required_codes=('opaque_base_operation',))
    for driver in ('sample.mjs','sample_test.py'):
        case('recognized opaque driver '+driver,extra={'test/tools/'+driver:''},required_codes=('opaque_driver',))
    for driver in ('sample.py','sample_test.mjs','sample.cjs','sample_test.cjs','sample.js','sample_test.js','sample.ts','sample_test.ts','sample-other.py'):
        case('other spelling fails closed '+driver,extra={'test/tools/'+driver:''},required_codes=('unrecognized_driver_spelling',))
    case('missing main','import Base\ndef value() -> Nat:\n  0n\n',required_codes=('missing_io_main',))
    case('quoted/commented main is absent','import Base\n# def main() -> IO(Unit):\ndef text() -> String:\n  "def main() -> IO(Unit):"\n',required_codes=('missing_io_main',))
    case('main with unsupported result signature','import Base\ndef main() -> Nat:\n  0n\n',required_codes=('missing_io_main',))
    case('multiple runnable mains',prefix+'def main() -> IO(Unit):\n  IO.print("ok duplicate")\n',required_codes=('ambiguous_io_main',))
    case('unsafe declaration',prefix+'\n@unsafe\n',required_codes=('unsafe',))
    case('custom effect import',prefix+'import ./effect.js as E\n',{'test/effect.js':''},('custom_effect',))
    case('Base alias',prefix.replace('import Base','import Base as B'),required_codes=('base_alias',))
    case('named import',prefix+'import Unknown as U\n',required_codes=('named_import',))
    case('unknown import syntax',prefix+'import ./bad name.bend as B\n',required_codes=('invalid_dependency_graph','unrecognized_import'))
    case('missing dependency',prefix+'import ./missing.bend as M\n',required_codes=('invalid_dependency_graph',))
    case('opaque effect through transitive module',prefix.replace('import Base\n','import Base\nimport ./leaf.bend as L\n'),
        {'test/leaf.bend':'import Base\ndef sentinel() -> Type:\n  IO.sleep\n'},('opaque_base_operation',))
    case('unsupported tool identity',tool_error='synthetic unknown identity refusal',required_codes=('unsupported_tool',))
    case('all reasons survive together','import Base\n@unsafe\ndef sentinel() -> Type:\n  IO.now\n',
        {'test/tools/sample.cjs':''},('missing_io_main','unsafe','opaque_base_operation','unrecognized_driver_spelling'))
    return {'passed':True,'cases':len(result),'results':result,
        'limits':'Metadata classifier assertions only; no fixture is compiled or accepted as a real test/check/proof pass.'}

control_result=controls();write('CONTROLS.json',control_result)
began=time.monotonic()
classifier=Classifier(SOURCE,error);tests=selector.all_tests(SOURCE)
def required_list(relative):
    path=SOURCE/relative
    if not path.exists():return {'path':relative,'present':False,'count':0,'tests':[]}
    names=sorted({line.split('#',1)[0].strip() for line in path.read_text().splitlines() if line.split('#',1)[0].strip()})
    assert set(names)<=set(tests),(relative,'list names unknown tests')
    return {'path':relative,'present':True,'sha256':digest(path),'count':len(names),'tests':names}
fast=required_list('scripts/fast-tests.txt');extended=required_list('scripts/extended-fast-tests.txt')
required=fast['tests'];assert required
records=[]
for test in tests:
    record=classifier.classify(test);record['required']=test in required
    record['extended_required']=test in extended['tests'];records.append(record)
histogram=Counter(code for record in records for code in record['reason_codes'])
operation_histogram=Counter(operation for record in records for operation in {op for values in record['opaque_Base_operations'].values() for op in values})

timing_path=SOURCE/'scripts/test-timings.txt';timings={};invalid=[];duplicates=set()
for line_no,line in enumerate(timing_path.read_text().splitlines(),1):
    if not line.strip() or line.lstrip().startswith('#'):continue
    try:
        value,name=line.strip().split(maxsplit=1);seconds=float(value)
        if not math.isfinite(seconds) or seconds<0:raise ValueError('not a finite nonnegative duration')
        if name in timings or name in duplicates:duplicates.add(name);timings.pop(name,None);continue
        timings[name]=seconds
    except ValueError as exc:invalid.append({'line':line_no,'detail':str(exc)})
def timing_share(entries):
    known=[r for r in entries if r['test'] in timings];eligible=[r for r in known if r['eligible']]
    total=sum(timings[r['test']] for r in known);part=sum(timings[r['test']] for r in eligible)
    return {'inventory_count':len(entries),'known_timing_count':len(known),'missing_timing_count':len(entries)-len(known),
        'known_seconds':total,'eligible_known_seconds':part,'eligible_known_timing_count':len(eligible),
        'eligible_share_of_known_seconds':part/total if total else None,
        'missing_timing_tests':[r['test'] for r in entries if r['test'] not in timings]}
timing_result={'path':'scripts/test-timings.txt','sha256':digest(timing_path),
    'source_comments':[line for line in timing_path.read_text().splitlines() if line.startswith('#')],
    'invalid_rows':invalid,'ambiguous_duplicate_tests':sorted(duplicates),
    'stale_timing_tests':sorted(set(timings)-set(tests)),
    'all_tests':timing_share(records),'required':timing_share([r for r in records if r['required']]),
    'extended_required':timing_share([r for r in records if r['extended_required']]),
    'limits':'Historical scheduling estimates, not timings on this source/worker and not a measured cache speedup. Missing durations are NOT imputed; percentages describe only known measured test-seconds, not full parallel-job wall time.'}
write('TIMING-SHARE.json',timing_result)
intact=inventory(SOURCE)==before and digest(SELF)==harness and digest(binary)==tool['sha256'] and selector.library_identity(binary.parent.parent/'bend2')==tool['library_sha256']
result={'kind':'deterministic standalone test-cache eligibility RESEARCH ONLY','source_revision':REV,'source_tree':TREE,
    'tool':tool,'tool_model_error':error,'selector_sha256':digest(SOURCE/'scripts/ci-select.py'),
    'classification_seconds':time.monotonic()-began,'tests':len(tests),'eligible':sum(r['eligible'] for r in records),
    'controls':{'passed':control_result['passed'],'cases':control_result['cases']},
    'required_inventory':fast,'extended_inventory':extended,
    'reason_histogram':dict(sorted(histogram.items())),'opaque_operation_histogram':dict(sorted(operation_histogram.items())),
    'histogram_limits':'Exclusion reasons are nonexclusive; every applicable reason is retained and each test counts once per reason code.',
    'timing_share':timing_result,
    'required':[r for r in records if r['required']],'records':records,'identity_intact':intact,'resources':resources(),
    'policy':'Conservative full-module closure; only console print/write native effects allowed. Clock/env/random/process/file/channel/spawn/custom effects/drivers opaque.',
    'limits':'Metadata design research only. Scanner controls passed, but eligibility is necessary, not sufficient, for deterministic native behavior or resource fit. No permission to skip; no test/compiler/proof execution or cache pass created. Native compiler/Bun verdicts and stack/memory bounds still need qualification; cached passes alone cannot observe a new resource regression.',
    'resource_regression_design':{'implemented':False,
        'identity':'A future cache key must bind compiler/Bun/OS/architecture, full source/driver/verdict identity and stack/memory/time execution profile; changed identities always require native execution.',
        'receipt':'Only owned admitted native execution may write a future pass, retaining wall time, peak RSS, cgroup peak/cap/events, CPU allocation and source/tool identities.',
        'sampling':'Proposed future policy: always rerun at least one deterministically rotating eligible test per build, plus a nightly complete from-scratch run. Reuse never certifies current resource behavior for unsampled tests.',
        'response':'A sample timeout, OOM, missing resource receipt or regression beyond a separately preregistered budget must fail closed and invalidate reuse for the execution profile. Sampling detects some regressions; only complete fresh execution observes every test.'},
    'test_executions':0,'compiler_executions':0,'proof_executions':0,'pass_records_created':0,'gate_qualified':False}
result['execution_counter_scope']='Native Bend regression/compiler/proof executions only. Python metadata classifier controls are counted separately in controls.'
write('RESULT.json',result)
print(json.dumps({k:result[k] for k in ('tests','eligible','classification_seconds','controls','reason_histogram','identity_intact','resources','gate_qualified')},sort_keys=True),flush=True)
print(json.dumps({'timing_share':{k:timing_result[k] for k in ('all_tests','required','extended_required')},
    'required':[{'test':r['test'],'eligible':r['eligible'],'reason_codes':r['reason_codes']} for r in result['required']]},sort_keys=True),flush=True)
shutil.rmtree(SOURCE)
assert intact and result['resources']['peak_bytes']<=536870912 and all(result['resources']['events'][k]==0 for k in ('oom','oom_kill','oom_group_kill'))
