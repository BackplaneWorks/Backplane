#!/usr/bin/env python3
"""Measure the exact-export contract using an ACTUAL qualified simulator cache.

No build, service or lock. Temporary Git commits are synthetic fixture history;
the real checkout is never changed. Corrupt artifacts are always restored.
Overall hot result stays incomplete while static coverage is unresolved.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('gate',ROOT/'scripts/ui-validate.py')
G=importlib.util.module_from_spec(spec);spec.loader.exec_module(G)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache-dir',required=True);p.add_argument('--out',required=True);a=p.parse_args()
    cache=Path(a.cache_dir).resolve();key,binding=G.identity(ROOT)
    G.reuse(cache,key)  # A forged unit-test receipt is never input here.
    receipt=cache/(key+'.json');module=cache/(key+'.mjs')
    rb,mb=receipt.read_bytes(),module.read_bytes();records=[]
    out=Path(a.out).resolve();out.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='bp-ui-contract-') as temp:
        work=Path(temp);repo=work/'repo';repo.mkdir()
        for name in G.files_under(ROOT):
            target=repo/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/name,target)
        def git(*args):
            return subprocess.check_output(['git','-c','core.hooksPath=/dev/null','-c','user.name=i2cjak',
                '-c','user.email=build@amemb.com',*args],cwd=repo,text=True).strip()
        git('init','-q');(repo/'delete-me.txt').write_text('public fixture');(repo/'rename-me.txt').write_text('public fixture')
        git('add','-A');git('commit','-qm','Synthetic immutable export base');base=git('rev-parse','HEAD')
        (repo/'delete-me.txt').unlink();git('mv','rename-me.txt','renamed.txt');git('add','-A');git('commit','-qm','Synthetic endpoints');head=git('rev-parse','HEAD')
        snapshot=work/'export';shutil.copytree(repo,snapshot,ignore=shutil.ignore_patterns('.git'))
        paths=work/'paths0'
        def endpoints(oid):
            paths.write_bytes(subprocess.check_output(['git','diff','--no-renames','--name-only','-z',base,oid],cwd=repo))
        endpoints(head)
        def inventory():return {str(p.relative_to(snapshot)):G.digest(p) for p in snapshot.rglob('*') if p.is_file()}
        before=inventory()
        def probe(label,budget=3000,environment=None,oid=head):
            report=work/'report.json';t=time.monotonic()
            r=subprocess.run([sys.executable,str(snapshot/'scripts/ui-validate.py'),'--snapshot',str(snapshot),
                '--base',base,'--head',oid,'--changed-paths0',str(paths),'--report',str(report),'--budget-ms',str(budget)],
                cwd=repo,env={**os.environ,'UI_VALIDATE_CACHE':str(cache),**(environment or {})},capture_output=True,text=True)
            elapsed=time.monotonic()-t;value=json.loads(report.read_text())
            records.append({'case':label,'exit':r.returncode,'wall_seconds':elapsed,'report':value})
            assert elapsed<3, (label,'exceeded contract budget')
            assert r.returncode in [1,3], 'computed coverage must never inherit simulator green'
        try:
            for i in range(3):probe('hot-'+str(i))
            r=json.loads(rb);r['result']['pass']=False;receipt.write_text(json.dumps(r));probe('corrupt-result');receipt.write_bytes(rb)
            module.write_bytes(mb[:-1]+bytes([mb[-1]^1]));probe('corrupt-module');module.write_bytes(mb)
            probe('runtime-options',environment={'NODE_OPTIONS':'--no-warnings'})
            probe('missing-compiler',environment={'PATH':'/usr/bin:/bin'})
            helper=snapshot/'src/web/view-delta.js';raw=helper.read_bytes();helper.write_bytes(raw+b'\n// synthetic changed pushed dependency\n')
            (repo/'src/web/view-delta.js').write_bytes(helper.read_bytes());git('add','-A');git('commit','-qm','Synthetic dependency mutation')
            changed=git('rev-parse','HEAD');endpoints(changed);probe('changed-pushed-helper',oid=changed)
            helper.write_bytes(raw);git('checkout','-q',head);endpoints(head)
            control=snapshot/'scripts/ahead-validation/ci.sh';mode=control.stat().st_mode;control.chmod(mode&~0o111);probe('changed-mode');control.chmod(mode)
            paths.write_bytes(b'delete-me.txt\0renamed.txt\0');probe('missing-rename-endpoint');endpoints(head)
            probe('deadline',1)
            assert inventory()==before and not list(snapshot.rglob('__pycache__')) and not (snapshot/'.git').exists()
        finally:
            receipt.write_bytes(rb);module.write_bytes(mb)
        result={'version':1,'sourceHead':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'fixtureBase':base,'fixtureHead':head,'key':key,'binding':binding,
            'receiptSha256':G.digest(receipt),'moduleSha256':G.digest(module),'immutableExport':True,'records':records}
        out.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'key':key,'cases':[(r['case'],r['exit'],round(r['wall_seconds'],3)) for r in records],'out':str(out)}))


if __name__=='__main__':main()
