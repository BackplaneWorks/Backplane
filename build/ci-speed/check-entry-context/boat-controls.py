"""Boat-only Python cache controls; mock fixtures are never proof/gate evidence."""
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import time
import unittest

REV='5b3f524240022cc1b38ce4f5a6d994244514eb48'
TREE='c9d7c3b39aa68901f7c5592f13032b33e6638d1d'
OUT=Path.cwd()/'build/cache-context-controls'


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda:f.read(65536),b''):h.update(chunk)
    return h.hexdigest()


def main():
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
    assert os.environ.get('BEND_NO_TELEMETRY')=='1'
    os.environ['PYTHONDONTWRITEBYTECODE']='1'
    at=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
    def resources():
        return {'memory_max_bytes':int((at/'memory.max').read_text()),'swap_max_bytes':int((at/'memory.swap.max').read_text()),
                'peak_bytes':int((at/'memory.peak').read_text()),
                'events':{k:int(v) for k,v in (line.split() for line in (at/'memory.events').read_text().splitlines())}}
    before=resources()
    assert before['memory_max_bytes']==536870912 and before['swap_max_bytes']==0
    assert not OUT.exists(),'attempt exists: inspect, never overwrite'
    OUT.mkdir(parents=True)
    repo=OUT/'fetch.git'
    subprocess.run(['git','init','-q','--bare',str(repo)],check=True)
    def git(*args):
        return subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=0','-c','pack.threads=1',
            '-c','pack.windowMemory=32m','-C',str(repo),*args],stderr=subprocess.PIPE,timeout=120)
    git('remote','add','origin','https://github.com/BackplaneWorks/Backplane.git')
    git('config','remote.origin.promisor','true');git('config','remote.origin.partialclonefilter','blob:none')
    git('fetch','--filter=blob:none','--no-tags','origin',REV)
    assert git('rev-parse','FETCH_HEAD').decode().strip()==REV
    assert git('rev-parse',REV+'^{tree}').decode().strip()==TREE
    source=OUT/'source';source.mkdir()
    archive=OUT/'source.tar'
    with archive.open('wb') as f:
        subprocess.run(['git','-C',str(repo),'archive',REV,'scripts','test/tools'],stdout=f,stderr=subprocess.PIPE,check=True,timeout=120)
    with tarfile.open(archive) as tar:tar.extractall(source,filter='data')
    archive.unlink();shutil.rmtree(repo)
    def identity():
        return {p.relative_to(source).as_posix():sha(p) for p in sorted(source.rglob('*')) if p.is_file()}
    original=identity()
    assert original['scripts/content-cache.py']=='faa23dd9217158ab560c48c9559a508e50b32646e51a1de1f31f9ee4c88e4424'
    (OUT/'INPUTS.json').write_text(json.dumps(original,sort_keys=True,indent=2)+'\n')
    # These cases deliberately use mock compilers/guards, inside temporary
    # fixtures, to verify control flow and byte/MAC boundaries. No native
    # Bend invocation, actual-repository pass recording or gate claim.
    names=['content_cache_test','cache_plan_snapshot_test','proof_cache_closure_test',
           'proof_cache_execution_test','cache_check_context_test']
    sys.path.insert(0,str(source/'test/tools'))
    suite=unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromName(name) for name in names)
    count=suite.countTestCases();assert count>=94,'incomplete control inventory'
    started=time.monotonic()
    with (OUT/'tests.txt').open('w') as log:
        result=unittest.TextTestRunner(stream=log,verbosity=2).run(suite)
    seconds=time.monotonic()-started
    after=resources();unchanged=identity()==original
    good=result.wasSuccessful() and unchanged and all(after['events'].get(k)==0 for k in ('max','oom','oom_kill','oom_group_kill'))
    status=Path('/proc/self/status').read_text()
    report={'kind':'Python cache routing/integrity controls; mock fixtures, NOT Bend proofs or gates',
            'head':REV,'tree':TREE,'helper_sha256':original['scripts/content-cache.py'],
            'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),
            'seconds':seconds,'success':good,'source_unchanged':unchanged,'before':before,'after':after,
            'proc_status':{line.split(':',1)[0]:line.split(':',1)[1].strip() for line in status.splitlines() if line.startswith(('VmHWM:','VmRSS:'))},
            'test_log_sha256':sha(OUT/'tests.txt'),'runner_sha256':sha(__file__),
            'native_bend_executions':0,'actual_repository_pass_seeds':0,'gate_qualified':False,
            'limits':'Cold check floor remains14GiB; no native execution, shared cache, CI, proof or build qualification.'}
    (OUT/'RESULT.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n')
    print(json.dumps(report,sort_keys=True),flush=True)
    shutil.rmtree(source)
    return 0 if good else 1


if __name__=='__main__':raise SystemExit(main())
