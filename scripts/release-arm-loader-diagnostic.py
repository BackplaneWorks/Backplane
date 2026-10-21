#!/usr/bin/env python3
"""Capture one loader diagnostic from the exact saved configured ARM build."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import zipfile

SOURCE = '8147b68dbc9bcddad7e3b51ae7768ed9eb211ecc'
TREE = '37c64023d084bca7f209db9d14a84f59d365f696'
RUN = 39623647353
ARTIFACT = 12357277029
ARCHIVE = '5f4b30a56061b5c00a258a8d2f8800bfc6547e2d40967b42d1474f8b54ac20d9'
BUILD = 'ee6f905f713acc7af9ae979a9513909e60c54ac7453ac56d6735c532928ca1be'
BINARY = '9accaca97d8ce7507550a1ee5e9acf93fb32aa3c73412301c59f79fb34ac4081'
COMPILE_METADATA = '2d6518da1f36263d3061746b37c3c8ce25b313f7'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def extract_original(archive, out):
    require(sha(archive) == ARCHIVE, 'Original configured artifact ZIP changed')
    require(not out.exists(), 'Fresh native extraction directory required')
    prefix = 'build/bootstrap-native/'
    with zipfile.ZipFile(archive) as z:
        chosen = []
        seen = set()
        for info in z.infolist():
            if prefix not in info.filename or info.is_dir():
                continue
            name = info.filename.split(prefix, 1)[1]
            p = PurePosixPath(name)
            require(not p.is_absolute() and str(p) == name and '\\' not in name and ':' not in name
                    and all(n not in ('', '.', '..') for n in name.split('/')), 'Unsafe native artifact path')
            require(name.casefold() not in seen, 'Duplicate native artifact path')
            seen.add(name.casefold())
            mode = info.external_attr >> 16
            require(not stat.S_ISLNK(mode) and not mode & 0o7002, 'Unsafe native artifact mode')
            require(info.file_size <= 512 * 1024**2, 'Native member exceeds bound')
            chosen.append((info, name, mode & 0o777 or 0o644))
        require(chosen and sum(i.file_size for i, _, _ in chosen) <= 1024**3, 'Native extraction bound')
        out.mkdir(parents=True)
        for info, name, mode in chosen:
            dest = out / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            with z.open(info) as src, dest.open('xb') as dst:
                shutil.copyfileobj(src, dst)
            dest.chmod(mode)
    require(sha(out/'BUILD.json') == BUILD and sha(out/'backplane-serve') == BINARY,
            'Original native BUILD or binary changed')
    record = json.loads((out/'BUILD.json').read_text())
    require((record['source_commit'], record['source_tree'], record['target'], record['version']) ==
            (SOURCE, TREE, 'linux-arm64', '0.12.0'), 'Original native source identity mismatch')
    return record


def diagnose(args):
    import platform
    require((platform.system(), platform.machine()) == ('Linux', 'aarch64'), 'Actual Linux ARM host required')
    source = args.source_root.resolve(strict=True)
    def git(*argv):
        return subprocess.check_output(['git', '-C', str(source), *argv], text=True).strip()
    require(git('rev-parse', 'HEAD') == SOURCE and git('rev-parse', 'HEAD^{tree}') == TREE
            and not git('status', '--porcelain', '--untracked-files=no'), 'Exact clean product source required')
    require(args.out.is_absolute() and not args.out.exists(), 'Fresh diagnostic directory required')
    args.out.mkdir(parents=True)
    native = args.out/'native'
    build = extract_original(args.archive, native)
    binary = native/'backplane-serve'
    loader = Path('/lib/ld-linux-aarch64.so.1')
    require(loader.resolve(strict=True).is_file() and str(loader.resolve()).startswith(('/lib/', '/usr/lib/')),
            'Expected system ARM loader required')
    preload = Path('/etc/ld.so.preload')
    require(not preload.exists() or not preload.read_text().strip(), 'Ambient preload file refused')
    env = {k:v for k,v in os.environ.items() if not k.startswith(
        ('LD_', 'DYLD_', 'BACKPLANE_GOOGLE_', 'BACKPLANE_TELEMETRY_', 'GH_TOKEN', 'GITHUB_TOKEN'))}
    env.update(LC_ALL='C', LANG='C')
    logs = args.out/'diagnostic';logs.mkdir()
    calls=[]
    for name, argv, extra in [
        ('program-headers', ['/usr/bin/readelf','--program-headers','--wide',str(binary)], {}),
        ('loader-debug', [str(loader),'--list',str(binary)], {'LD_DEBUG':'files'})]:
        cp = subprocess.run(argv, env={**env,**extra}, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=20, close_fds=True)
        (logs/(name+'.stdout')).write_bytes(cp.stdout);(logs/(name+'.stderr')).write_bytes(cp.stderr)
        calls.append({'argv':argv,'environmentOverrides':extra,'exit':cp.returncode,
                      'toolSha256':sha(argv[0]),'stdoutSha256':sha(logs/(name+'.stdout')),
                      'stderrSha256':sha(logs/(name+'.stderr'))})
        require(cp.returncode==0,'Bounded loader diagnostic failed: '+name)
    require(sha(native/'BUILD.json')==BUILD and sha(binary)==BINARY,'Original native bytes changed')
    require(not git('status','--porcelain','--untracked-files=no'),'Product source changed')
    result={'schema':1,'status':'ACTUAL_ARM_LOADER_DIAGNOSTIC_ONLY_NO_COMPONENT_ACCEPTANCE',
            'sourceCommit':SOURCE,'sourceTree':TREE,'originalRunId':RUN,'originalArtifactId':ARTIFACT,
            'originalArchiveSha256':ARCHIVE,'originalBuildSha256':BUILD,'binarySha256':BINARY,
            'originalCompileMetadataCommit':COMPILE_METADATA,'recoveryMetadataCommit':os.environ.get('GITHUB_SHA'),
            'compileInputs':build['compileInputs'],'calls':calls,'nativeCompilationRepeated':False,
            'serviceStarted':False,'runtimeAcceptance':'PENDING'}
    (args.out/'DIAGNOSTIC.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result),flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source-root', type=Path, required=True)
    p.add_argument('--archive', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    diagnose(p.parse_args())


if __name__ == '__main__':
    main()
