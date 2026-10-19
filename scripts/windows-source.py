#!/usr/bin/env python3
"""Bind Windows emitted inputs to one clean source revision and version."""
from pathlib import Path
import hashlib,json,os,subprocess,sys

root=Path(__file__).resolve().parent.parent
directory=root/'build/windows-emitted'
names=('backplane.c.gz','runtime.c.gz','providers.c.gz')

def git(*args):
    return subprocess.check_output(['git',*args],cwd=root,text=True).strip()

def channel_for(version):
    channel='nightly' if '-nightly.' in version.removeprefix('v') else 'release'
    if os.environ.get('BACKPLANE_TELEMETRY_CHANNEL',channel) != channel:
        raise SystemExit('windows-source: telemetry channel disagrees with the package version')
    return channel

def identity():
    if git('status','--porcelain'):
        raise SystemExit('windows-source: source checkout must be clean')
    version=os.environ.get('BACKPLANE_VERSION','')
    channel=channel_for(os.environ.get('BACKPLANE_VERSION_STAMP') or version)
    return {'schema':2,'commit':git('rev-parse','HEAD'),
            'tree':git('rev-parse','HEAD^{tree}'),
            'version':version,'telemetry_channel':channel,
            'inputs_sha256':{name:hashlib.sha256((directory/name).read_bytes()).hexdigest() for name in names}}

def check_receipt():
    expected=identity()
    if json.loads((directory/'SOURCE.json').read_text()) != expected:
        raise SystemExit('windows-source: emitted source, version, channel or inputs do not match checkout')
    return expected

def main():
    receipt=directory/'SOURCE.json'
    if sys.argv[1:] == ['record']:
        receipt.write_text(json.dumps(identity(),indent=2)+'\n')
    elif sys.argv[1:] == ['check']:
        expected=check_receipt()
        print('WINDOWS_EMITTED_SOURCE_VERSION_CHANNEL_AND_INPUTS_MATCH',expected['commit'])
    else:
        raise SystemExit('usage: windows-source.py record|check')

if __name__=='__main__':main()
