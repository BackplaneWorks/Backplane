#!/usr/bin/env python3
"""Historical negative controls, reading real git objects. --emit is heavy.

Each old program is emitted from its own source tree, never a reconstructed
bug or a replacement implementation. Outputs live only under build/.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'build/ahead-validation/history'
REVISIONS={'unread':'f85e2da01^','pending':'1b1c094a0^','surrogate':'c81ca7506^'}


def stage(revision,label):
    folder=OUT/label;folder.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryFile() as f:
        subprocess.run(['git','archive',revision,'src'],cwd=ROOT,stdout=f,check=True);f.seek(0)
        with tarfile.open(fileobj=f) as archive:archive.extractall(folder,filter='data')
    (folder/'test').mkdir(exist_ok=True)
    (folder/'test/ahead_validation_test.bend').write_bytes((ROOT/'test/ahead_validation_test.bend').read_bytes())
    return folder


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--emit',action='store_true');a=p.parse_args()
    OUT.mkdir(parents=True,exist_ok=True);results={}
    spec=importlib.util.spec_from_file_location('coverage',ROOT/'scripts/ahead-validation/coverage.py')
    c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
    for label,rev in [('copy-before','0020fbc6e^'),('copy-after','0020fbc6e'),('unread-before','f85e2da01^'),('current','HEAD')]:
        r=c.inventory(c.Source(rev));results[label]={'revision':subprocess.check_output(['git','rev-parse',rev],cwd=ROOT,text=True).strip(),
            'static':{k:v for k,v in r.items() if k not in ['actions','urls','computed_actions']},'computed':len(r['computed_actions'])}
    for label,rev in REVISIONS.items():
        folder=stage(rev,label);module=folder/'ui.mjs';start=time.monotonic()
        if a.emit:
            with (folder/'emit.log').open('w') as log:
                r=subprocess.run(['bend','test/ahead_validation_test.bend','-o','ui.mjs'],cwd=folder,
                    env={**os.environ,'BEND_NO_TELEMETRY':'1','BUN_JSC_forceRAMSize':'4294967296','BUN_JSC_maxPerThreadStackUsage':'536870912'},stdout=log,stderr=subprocess.STDOUT)
            if r.returncode:
                results[label]={'revision':rev,'emitFailed':True,'seconds':time.monotonic()-start};continue
        if module.exists():
            r=subprocess.run(['node','scripts/ahead-validation/history-properties.mjs',str(module)],cwd=ROOT,capture_output=True,text=True,check=True)
            results[label]={'revision':subprocess.check_output(['git','rev-parse',rev],cwd=ROOT,text=True).strip(),'result':json.loads(r.stdout),'emit_seconds':time.monotonic()-start}
            if label=='unread':
                transport=subprocess.run(['node','scripts/ahead-validation/transport.mjs',str(folder)],cwd=ROOT,capture_output=True,text=True)
                results[label]['transport']=json.loads(transport.stdout)
        else:results[label]={'revision':rev,'status':'not-emitted'}
    (OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(results))


if __name__=='__main__':main()
