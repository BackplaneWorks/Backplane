#!/usr/bin/env python3
"""Recover only matching-host ABI measurement from the saved configured ARM build."""
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
COMPONENT_PRODUCER = 'b6350f66cee22adaca71f52f52105a8a6dbdb3fe2b6ba48b95d6e66e65ef3cc7'


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


def recover(args):
    source = args.source_root.resolve(strict=True)
    def git(*argv):
        return subprocess.check_output(['git', '-C', str(source), *argv], text=True).strip()
    require(git('rev-parse', 'HEAD') == SOURCE and git('rev-parse', 'HEAD^{tree}') == TREE,
            'Exact clean product source required')
    require(not git('status', '--porcelain', '--untracked-files=no'), 'Tracked product changed')
    runtime = args.runtime_producer.resolve(strict=True)
    producer = runtime.with_name('service-release.py')
    require(re.fullmatch('[0-9a-f]{64}', args.runtime_sha256) and sha(runtime) == args.runtime_sha256,
            'Qualified runtime factory pin required')
    require(sha(producer) == COMPONENT_PRODUCER, 'Original component producer changed')
    require(args.out.is_absolute() and not args.out.exists(), 'Fresh recovery output required')
    args.out.mkdir(parents=True)
    native = args.out/'native'
    build = extract_original(args.archive, native)
    metadata = subprocess.check_output(['git', '-C', str(runtime.parent.parent), 'rev-parse', 'HEAD'], text=True).strip()
    require(metadata == os.environ.get('GITHUB_SHA', metadata), 'Recovery metadata checkout mismatch')
    record = {'schema': 1, 'kind': 'backplane-original-native-runtime-recovery',
              'sourceCommit': SOURCE, 'sourceTree': TREE, 'originalRunId': RUN,
              'originalArtifactId': ARTIFACT, 'originalArchiveSha256': ARCHIVE,
              'originalCompileMetadataCommit': COMPILE_METADATA, 'recoveryMetadataCommit': metadata,
              'originalBuildSha256': BUILD, 'binarySha256': BINARY,
              'compileInputs': build['compileInputs'], 'runtimeFactorySha256': sha(runtime),
              'componentProducerSha256': sha(producer), 'nativeCompilationRepeated': False,
              'proofRepeated': False, 'emissionRepeated': False, 'runtimeAcceptance': 'PENDING'}
    receipt = args.out/'RECOVERY.json'
    receipt.write_text(json.dumps(record, indent=2)+'\n')
    subprocess.run([sys.executable, str(runtime), '--source-root', str(source), '--build', str(native),
                    '--producer', str(producer), '--producer-sha256', COMPONENT_PRODUCER,
                    '--out', str(args.out/'bootstrap-service')], check=True)
    require(sha(native/'BUILD.json') == BUILD and sha(native/'backplane-serve') == BINARY,
            'Recovery mutated original native bytes')
    require(not git('status', '--porcelain', '--untracked-files=no'), 'Recovery mutated product')
    result = json.loads((args.out/'bootstrap-service/RESULT.json').read_text())
    record.update(status='ORIGINAL_NATIVE_COMPONENT_RECOVERED', result=result)
    receipt.write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(record), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source-root', type=Path, required=True)
    p.add_argument('--archive', type=Path, required=True)
    p.add_argument('--runtime-producer', type=Path, required=True)
    p.add_argument('--runtime-sha256', required=True)
    p.add_argument('--out', type=Path, required=True)
    recover(p.parse_args())


if __name__ == '__main__':
    main()
