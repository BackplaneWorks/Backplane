#!/usr/bin/env python3
"""Bind Windows emitted inputs to one clean source revision and version."""
from pathlib import Path
import hashlib,json,os,subprocess,sys

root=Path(__file__).resolve().parent.parent
directory=root/'build/windows-emitted'
names=('backplane.c.gz','runtime.c.gz','providers.c.gz')

def git(*args):
    return subprocess.check_output(['git',*args],cwd=root,text=True).strip()

def identity():
    if git('status','--porcelain'):
        raise SystemExit('windows-source: source checkout must be clean')
    return {'schema':1,'commit':git('rev-parse','HEAD'),
            'tree':git('rev-parse','HEAD^{tree}'),
            'version':os.environ.get('BACKPLANE_VERSION',''),
            'inputs_sha256':{name:hashlib.sha256((directory/name).read_bytes()).hexdigest() for name in names}}

def main():
    receipt=directory/'SOURCE.json'
    expected=identity()
    if sys.argv[1:] == ['record']:
        receipt.write_text(json.dumps(expected,indent=2)+'\n')
    elif sys.argv[1:] == ['check']:
        if json.loads(receipt.read_text()) != expected:
            raise SystemExit('windows-source: emitted source, version or inputs do not match checkout')
        print('WINDOWS_EMITTED_SOURCE_VERSION_AND_INPUTS_MATCH',expected['commit'])
    else:
        raise SystemExit('usage: windows-source.py record|check')

if __name__=='__main__':main()
