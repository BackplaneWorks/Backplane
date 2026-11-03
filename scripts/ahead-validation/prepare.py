#!/usr/bin/env python3
"""Explicit cold CI preparation. Run under Backplane's bend lock or on Boat.

ui-validate NEVER invokes this command. It emits the real pure functions,
executes all required fixed seeds, checks bytes before and after, then writes
an atomic content-bound receipt. An emit by itself is never a passing result.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('ui_validate',ROOT/'scripts/ui-validate.py')
gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)


def emission_identity(binding):
    # A remote emit can be adopted only after identical compiler/source
    # verification. Tests then run AGAIN locally under the actual local
    # Node/Python identity; a remote runtime receipt never becomes local PASS.
    return {'inputs':{k:(v['sha256'] if isinstance(v,dict) else v) for k,v in binding['inputs'].items()
                      if (k.startswith('src/') and k.endswith(('.bend','.js','.c','.h'))) or k=='test/ahead_validation_test.bend'},
            'compiler':{k:v for k,v in binding['tools']['bend'].items() if k!='path'},
            'named_library':binding['tools']['named_library'],
            'build_environment':gate.SPEC['buildEnvironment']}


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache-dir',required=True)
    p.add_argument('--module',help='explicit adoption of a verified existing emit; never builds')
    p.add_argument('--emission-receipt');a=p.parse_args()
    if bool(a.module)!=bool(a.emission_receipt):p.error('--module and --emission-receipt are paired')
    cache=Path(a.cache_dir).resolve();cache.mkdir(parents=True,exist_ok=True)
    build=ROOT/'build/ahead-validation';build.mkdir(parents=True,exist_ok=True)
    key,binding=gate.identity(ROOT)
    start=time.monotonic();module=Path(a.module).resolve() if a.module else build/'prepared.mjs'
    emission=emission_identity(binding)
    if a.module:
        record=json.loads(Path(a.emission_receipt).read_text())
        checksum=record.pop('receipt_sha256')
        if (gate.sha(gate.canonical(record))!=checksum or record['binding']!=emission or
                record['module_sha256']!=gate.digest(module)):
            raise SystemExit('existing emission identity mismatch; no local receipt')
        emit_seconds=0
    else:
        with (build/'prepare-emit.log').open('w') as log:
            subprocess.run(['bend','test/ahead_validation_test.bend','-o',str(module)],cwd=ROOT,
                           env={**os.environ,**gate.SPEC['buildEnvironment']},stdout=log,stderr=subprocess.STDOUT,check=True)
        emit_seconds=time.monotonic()-start
    result=subprocess.run(['node','test/tools/ahead_validation.mjs',str(module),'--out',str(build)],cwd=ROOT,capture_output=True,text=True,check=True)
    print(result.stdout,end='')
    value=json.loads(result.stdout.strip().splitlines()[-1]);assert value['pass'] is True
    def lane(script):
        args=['node',script]+([str(module)] if script.endswith('trace.mjs') else [])
        r=subprocess.run(args,cwd=ROOT,capture_output=True,text=True,check=True)
        print(r.stdout,end='');v=json.loads(r.stdout.strip().splitlines()[-1]);assert v['pass'] is True;return v
    transport=lane('scripts/ahead-validation/transport.mjs');trace=lane('scripts/ahead-validation/trace.mjs')
    after,_=gate.identity(ROOT)
    if after!=key:raise SystemExit('inputs changed during cold preparation; no receipt')
    emitted={'binding':emission,'module_sha256':gate.digest(module)}
    emitted['receipt_sha256']=gate.sha(gate.canonical(emitted))
    (build/'emission.json').write_text(json.dumps(emitted,indent=2)+'\n')
    target=cache/(key+'.mjs');shutil.copyfile(module,target)
    receipt={'key':key,'spec':gate.SPEC,'binding':binding,'result':value,'result_sha256':gate.sha(gate.canonical(value)),
             'module_sha256':gate.digest(target),'transport':transport,'trace':trace,
             'lanes_sha256':gate.sha(gate.canonical([transport,trace])),'emit_seconds':emit_seconds}
    receipt['receipt_sha256']=gate.sha(gate.canonical(receipt))
    temp=cache/(key+'.tmp');temp.write_text(json.dumps(receipt,indent=2)+'\n');temp.replace(cache/(key+'.json'))
    print(json.dumps({'key':key,'emit_seconds':emit_seconds,'total_seconds':time.monotonic()-start}))


if __name__=='__main__':main()
