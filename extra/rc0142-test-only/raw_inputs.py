"""Verify experimental raw inputs for TEST packaging; never claim stock equivalence.

This is a separate consumer. It does not replace service-release.verified_preemission.
No compiler or emitter is executed here.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile

COMMIT = 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4'
TREE = 'ec6c85b4a8b62a0227f09151a9ec52a362b20ae2'
TOOLCHAIN_SHA = '29d715ad67c69722f67b916fd35ce286160f88ec382fe92748840a1943cf4006'
ARCHIVE_SHA = 'be39bfb8c44da887702c51ef80dea07a6b0366c754f2eb15615e73a1f0d62581'
BUN_SHA = '795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822'
WORKER_SHA = '141b854565f7370fb2b959ef545d9255824b3487f73ab0295829b3216000dabd'
TEST_STATUS = 'TEST_ONLY_UNQUALIFIED_SERVICE_BUILD'


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def regular(path):
    path = Path(path).absolute()
    need(not any(p.is_symlink() for p in [path, *path.parents]), 'Input symlink refused')
    need(path.is_file(), 'Regular input file required: ' + str(path))
    return path


def reference(path):
    path = regular(path)
    return {'path': str(path), 'sha256': sha(path), 'bytes': path.stat().st_size}


def member(name):
    p = PurePosixPath(name)
    need(isinstance(name, str) and name and not p.is_absolute()
         and str(p) == name and '\\' not in name and ':' not in name
         and all(x not in ('', '.', '..') for x in name.split('/')), 'Unsafe input member')
    return name


def verify(receipt, expected_sha, source, toolchain, archive, worker):
    receipt, toolchain, archive, worker = map(regular, (receipt, toolchain, archive, worker))
    source = Path(source).resolve(strict=True)
    need(sha(receipt) == expected_sha, 'Authoritative collected receipt differs')
    need(sha(toolchain) == TOOLCHAIN_SHA and sha(archive) == ARCHIVE_SHA
         and sha(worker) == WORKER_SHA, 'Experimental toolchain/flags-worker identity differs')
    tools = read(toolchain)
    need(tools['preferredcompiler_mode'] == 'composite-settle-41262875'
         and tools['archive_sha256'] == ARCHIVE_SHA
         and tools['bun_binary_sha256'] == BUN_SHA, 'Experimental identity differs')
    files = tools['compiler_files']
    need(len(files) == 125, 'Complete 125-file compiler manifest required')
    actual_compiler = {}
    with tarfile.open(archive) as tar:
        for item in tar.getmembers():
            if item.isdir():
                continue
            need(item.isfile() and not item.issparse(), 'Compiler archive links/special files refused')
            name = member(item.name)
            need(name.startswith('compiler/'), 'Unexpected compiler archive root')
            name = name.removeprefix('compiler/')
            need(name not in actual_compiler, 'Duplicate compiler member')
            with tar.extractfile(item) as stream:
                actual_compiler[name] = hashlib.file_digest(stream, 'sha256').hexdigest()
    need(actual_compiler == files, 'Compiler archive inventory differs from all125 pins')
    def git(*args):
        return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()
    need(git('rev-parse', 'HEAD') == COMMIT and git('rev-parse', 'HEAD^{tree}') == TREE,
         'Frozen source commit/tree differs')
    need(not git('status', '--porcelain', '--untracked-files=no'), 'Tracked product source changed')
    raw = read(receipt)
    need((raw.get('kind'), raw.get('status'), raw.get('sourceCommit'), raw.get('sourceTree'),
          raw.get('version'), raw.get('channel'), raw.get('releaseQualified')) ==
         ('backplane-experimental-source-compiler-emission',
          'COMPLETE_RAW_BYTES_STOCK_COMPARISON_REQUIRED', COMMIT, TREE, '0.14.2', 'release', False),
         'Actual successful experimental raw receipt required')
    need(raw['compiler'] == tools, 'Producer compiler manifest differs')
    root = receipt.parent
    inventory = {}
    for path in sorted(root.rglob('*')):
        need(not path.is_symlink(), 'Raw member symlink refused')
        if path.is_file() and path != receipt:
            inventory[member(path.relative_to(root).as_posix())] = {
                'sha256': sha(path), 'bytes': path.stat().st_size}
    need(inventory == raw['files'], 'Raw source/output/log inventory differs')
    source_manifest = read(root / 'SOURCE.json')
    need(source_manifest == {'commit': COMMIT, 'tree': TREE,
         'members': {n: sha(source / n) for n in git('ls-files').splitlines()}},
         'Complete frozen source manifest differs')
    phases = read(root / 'LOWMEM-PHASES.json')
    need([p.get('phase') for p in phases] == ['server', 'web']
         and all(p.get('exitCode') == 0 and p.get('hardMemoryCapBytes') <= 8589934592
                 and p.get('swapMaxBytes') == 0 for p in phases), 'Hub/web phase failed/missing')
    for name in ('server', 'web'):
        end = read(root / 'logs' / (name + '.EXIT.json'))
        need(end['exitCode'] == 0 and end['compilerPostIdentity'] is True,
             'Actual compiler exit/postidentity missing')
    need(regular(root / 'server.js').stat().st_size > 0
         and regular(root / 'web/index.html').stat().st_size > 0
         and list((root / 'web').glob('chunk-*.js')), 'Actual hub/web bytes missing')
    identity = {'schema': 1, 'kind': 'backplane-test-only-raw-input-binding',
        'sourceCommit': COMMIT, 'sourceTree': TREE, 'version': '0.14.2', 'channel': 'release',
        'compilerMode': tools['preferredcompiler_mode'], 'compilerManifest': reference(toolchain),
        'compilerArchive': reference(archive), 'compilerFileCount': 125,
        'compilerFiles': files, 'emitterBunSHA256': BUN_SHA, 'flagsWorker': reference(worker),
        'rawReceipt': reference(receipt), 'rawServer': reference(root / 'server.js'),
        'sourceManifest': reference(root / 'SOURCE.json'),
        'rawWebMembers': {n: v for n, v in inventory.items() if n.startswith('web/')},
        'releaseQualified': False, 'stockByteComparison': 'NOT_PERFORMED_ON_AFA',
        'knownDifference': '32-byte f32_round serialization difference on518; no fullafa comparison',
        'publication': 'HOLD_FOR_OUTPUT_REVIEW_AND_USER_CONFIRMATION'}
    canonical = json.dumps(identity, sort_keys=True, separators=(',', ':')).encode()
    identity['rawInputBindingKey'] = hashlib.sha256(canonical).hexdigest()
    return identity


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('receipt', 'source', 'toolchain', 'compiler-archive', 'worker', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--receipt-sha256', required=True)
    a = p.parse_args()
    value = verify(a.receipt, a.receipt_sha256, a.source, a.toolchain, a.compiler_archive, a.worker)
    with a.output.open('x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')
    print(json.dumps({'status': 'TEST_ONLY_RAW_INPUTS_VERIFIED', 'binding': reference(a.output),
                      'releaseQualified': False}), flush=True)


if __name__ == '__main__':
    main()
