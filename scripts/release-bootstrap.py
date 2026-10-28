#!/usr/bin/env python3
"""Transport source-bound public build inputs into configured release producers.

No publication, secret fetching, target acceptance or automatic install.
Only the checked-out producers execute; packet files are data, never scripts.
The JS service is cross-built for every target onto the target's pinned Bun runtime.
"""
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
import tarfile
import tempfile
import urllib.parse
import urllib.request
import zipfile

VERSION = '0.13.0'
OPERATIONS = ('js', 'android')
MAX_BYTES = 4 * 1024**3
# The Bun every JS service is built with, and each target's runtime it is
# linked onto (`bun build --compile --compile-executable-path`): the release
# zips and their SHASUMS256.txt digests.
BUN_VERSION = '1.4.3'
BUN_RUNTIMES = {
    'linux-x64': ('bun-linux-x64', '3e730528d89775f03f87f05c06787c4df4099d0a37f3f252e7b829cb23816e18'),
    'linux-arm64': ('bun-linux-aarch64', 'efa9813da5ed72423bf847f916e8d2c47c0d776add972354026a75e10da9aa21'),
    'darwin-x64': ('bun-darwin-x64', '62660e6438eb872278877acca77e1152d5d031324df368d4a81b428211489d2d'),
    'darwin-arm64': ('bun-darwin-aarch64', '80afd4c069b46a6fa8f3ec520bebc750a7843e4a8d5b9583d29ad436c00a3403'),
    'windows-x64': ('bun-windows-x64', '0bf84e808552beea67bf1202a287a35c6c825d837b287b3f8e9dd513dba35d3d'),
}
JS_INPUTS = 'js/JS-INPUTS.json'


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def relative(name):
    need(isinstance(name, str) and name and '\\' not in name and ':' not in name, 'noncanonical input path')
    p = PurePosixPath(name)
    need(not p.is_absolute() and str(p) == name and all(x not in ('', '.', '..') for x in name.split('/')), 'unsafe input path')
    return p


def source_identity(source, checkout):
    need(re.fullmatch('[0-9a-f]{40}', source), 'full source commit required')
    def git(*args):
        return subprocess.check_output(['git', '-C', str(checkout), *args], text=True).strip()
    need(git('rev-parse', 'HEAD') == source, 'checkout differs from frozen source')
    need(not git('status', '--porcelain', '--untracked-files=no'), 'tracked source changed')
    return git('rev-parse', 'HEAD^{tree}')


def record_execution(checkout, source, tree):
    metadata = Path(__file__).resolve().parents[1]
    workflow = subprocess.check_output(['git', '-C', str(metadata), 'rev-parse', 'HEAD'], text=True).strip()
    expected = os.environ.get('GITHUB_SHA')
    need(not expected or expected == workflow, 'workflow tool checkout differs from actual workflow SHA')
    record = {'schema': 1, 'kind': 'backplane-bootstrap-execution', 'productSourceCommit': source,
              'productSourceTree': tree, 'workflowSourceCommit': workflow,
              'helperSha256': sha(Path(__file__).resolve()), 'version': VERSION,
              'channel': 'release', 'configuredBuildAccepted': False}
    p = checkout / 'build/bootstrap-execution.json'
    p.parent.mkdir(parents=True, exist_ok=True)
    if p.exists():
        need(json.loads(p.read_text()) == record, 'bootstrap execution provenance changed')
    else:
        p.write_text(json.dumps(record, indent=2) + '\n')
    return record


def inventory(directory):
    result = {}
    folded = set()
    for p in sorted(directory.rglob('*')):
        name = p.relative_to(directory).as_posix()
        relative(name)
        need(not p.is_symlink(), 'packet symlinks are refused')
        need(name.casefold() not in folded, 'case-colliding packet member')
        folded.add(name.casefold())
        if p.is_dir():
            continue
        need(p.is_file(), 'packet special file refused')
        need(not name.lower().endswith(('.pem', '.key', '.p12', '.pfx', '.keystore', '.jks', '.mobileprovision')),
             'private signing material is not a bootstrap input')
        result[name] = {'sha256': sha(p), 'bytes': p.stat().st_size}
    return result


def verify(directory, source, tree, operation):
    manifest = json.loads((directory / 'BOOTSTRAP.json').read_text())
    need((manifest.get('schema'), manifest.get('kind'), manifest.get('sourceCommit'), manifest.get('sourceTree'),
          manifest.get('version'), manifest.get('channel'), manifest.get('operation')) ==
         (1, 'backplane-public-build-inputs', source, tree, VERSION, 'release', operation), 'bootstrap identity mismatch')
    actual = inventory(directory)
    actual.pop('BOOTSTRAP.json', None)
    need(actual and actual == manifest.get('files'), 'bootstrap input bytes differ')
    if operation == 'js':
        js_inputs(directory, source, tree)
    else:
        r = json.loads((directory / 'android-release-inputs.json').read_text())
        need((r.get('sourceCommit'), r.get('sourceTree', tree)) == (source, tree), 'producer request source mismatch')
    return manifest


def js_inputs(directory, source, tree):
    # The source qualification, web notices and the external notice files the
    # web build copies: all public, all bound to the frozen source.
    r = json.loads((directory / JS_INPUTS).read_text())
    need((r.get('schema'), r.get('kind'), r.get('sourceCommit'), r.get('sourceTree'), r.get('version')) ==
         (1, 'backplane-service-js-bootstrap-inputs', source, tree, VERSION), 'JS producer inputs source mismatch')
    for key in ('sourceQualification', 'webNotices'):
        need((directory / relative(r.get(key, ''))).is_file(), 'JS producer input missing: ' + key)
    notices = r.get('noticeInputs', {})
    need(isinstance(notices, dict), 'notice inputs must be a mapping')
    for dest, member in notices.items():
        # only into the ignored build folder, never over product source
        need(str(relative(dest)).startswith('build/release-notice-inputs/'), 'notice input outside build/release-notice-inputs')
        need((directory / relative(member)).is_file(), 'notice input missing')
    return r


def pack(directory, output, source, tree, operation):
    need(directory.is_dir() and not output.exists(), 'fresh packet output required')
    need(not (directory / 'BOOTSTRAP.json').exists(), 'input directory must not contain generated manifest')
    files = inventory(directory)
    need(files, 'empty packet refused')
    manifest = {'schema': 1, 'kind': 'backplane-public-build-inputs', 'sourceCommit': source,
                'sourceTree': tree, 'version': VERSION, 'channel': 'release', 'operation': operation, 'files': files}
    with tempfile.TemporaryDirectory(prefix='bp-bootstrap-pack-') as tmp:
        meta = Path(tmp) / 'BOOTSTRAP.json'
        meta.write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n')
        output.parent.mkdir(parents=True, exist_ok=True)
        with tarfile.open(output, 'w:gz') as tar:
            tar.add(meta, arcname='BOOTSTRAP.json', recursive=False)
            for name in files:
                tar.add(directory / name, arcname=name, recursive=False)
    return {'archiveSha256': sha(output), 'archiveBytes': output.stat().st_size, **{k: manifest[k] for k in ('sourceCommit', 'sourceTree', 'operation')}}


def extract(archive, output):
    need(not output.exists(), 'fresh extraction directory required')
    with tarfile.open(archive, 'r:gz') as tar:
        members = tar.getmembers()
        names = set()
        total = 0
        for m in members:
            relative(m.name)
            need(m.isfile() and not m.issparse(), 'only regular packet files are allowed')
            need(not m.mode & 0o7002, 'privileged or world-writable packet mode refused')
            need(m.name.casefold() not in names, 'duplicate packet member')
            names.add(m.name.casefold())
            total += m.size
            need(0 <= m.size <= MAX_BYTES and total <= MAX_BYTES, 'packet size limit exceeded')
        output.mkdir(parents=True)
        for m in members:
            p = output / m.name
            p.parent.mkdir(parents=True, exist_ok=True)
            with tar.extractfile(m) as src, p.open('xb') as dst:
                shutil.copyfileobj(src, dst)
            # Producer receipts bind modes as well as bytes (local emits use 0664).
            p.chmod(m.mode & 0o777)


def download(url, expected, archive):
    u = urllib.parse.urlsplit(url)
    need(u.scheme == 'https' and u.hostname and not u.username and not u.password and not u.fragment and
         u.port in (None, 443), 'HTTPS artifact URL without credentials required')
    need(re.fullmatch('[0-9a-f]{64}', expected), 'trusted packet digest required')
    # No account headers, cookies, config or secret environment is forwarded.
    with urllib.request.urlopen(urllib.request.Request(url), timeout=120) as response, archive.open('wb') as f:
        need(urllib.parse.urlsplit(response.url).scheme == 'https', 'insecure download redirect refused')
        count = 0
        for chunk in iter(lambda: response.read(1024 * 1024), b''):
            count += len(chunk)
            need(count <= MAX_BYTES, 'download size limit exceeded')
            f.write(chunk)
    need(sha(archive) == expected, 'download differs from trusted digest')


def fetch(url, expected, output, source, tree, operation):
    with tempfile.TemporaryDirectory(prefix='bp-bootstrap-fetch-') as tmp:
        archive = Path(tmp) / 'inputs.tar.gz'
        download(url, expected, archive)
        extract(archive, output)
    return verify(output, source, tree, operation)


def tool(name, flag):
    found = shutil.which(name)
    need(found, name + ' is not on PATH')
    p = Path(found).resolve(strict=True)
    v = subprocess.check_output([str(p), flag], text=True, stderr=subprocess.STDOUT).strip()
    return {'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p), 'version': v}


def bun_runtime(target, destination):
    # The target's own Bun, from its pinned release zip; only its one executable is taken.
    need(target in BUN_RUNTIMES, 'JS service target required')
    stem, expected = BUN_RUNTIMES[target]
    exe = 'bun.exe' if target.startswith('windows-') else 'bun'
    need(not destination.exists(), 'fresh Bun runtime directory required')
    destination.mkdir(parents=True)
    archive = destination / (stem + '.zip')
    download('https://github.com/oven-sh/bun/releases/download/bun-v' + BUN_VERSION + '/' + stem + '.zip', expected, archive)
    with zipfile.ZipFile(archive) as z:
        info = z.getinfo(stem + '/' + exe)
        need(0 < info.file_size <= MAX_BYTES, 'Bun runtime size limit exceeded')
        path = destination / exe
        with z.open(info) as src, path.open('xb') as dst:
            shutil.copyfileobj(src, dst)
    path.chmod(0o755)
    archive.unlink()
    return {'path': str(path), 'sha256': sha(path), 'bytes': path.stat().st_size,
            'version': BUN_VERSION, 'target': target, 'archiveSha256': expected}


def run(directory, operation, target, source, checkout):
    tree = source_identity(source, checkout)
    verify(directory, source, tree, operation)
    if operation == 'js':
        need(target in BUN_RUNTIMES, 'JS service target required')
        inputs = js_inputs(directory, source, tree)
        for dest, member in inputs.get('noticeInputs', {}).items():
            path = checkout / relative(dest)
            if path.exists():
                need(path.is_file() and sha(path) == sha(directory / relative(member)), 'notice input differs in checkout')
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(directory / relative(member), path)
        bun = tool('bun', '--version')
        need(bun['version'] == BUN_VERSION, 'Bun ' + BUN_VERSION + ' required')
        r = {'schema': 1, 'kind': 'backplane-service-js-inputs', 'target': target,
             'release': {'version': VERSION, 'tag': 'v' + VERSION, 'channel': 'release', 'commit': source, 'tree': tree},
             'toolchain': {'bend': tool('bend', 'version'), 'bun': bun,
                           'bunRuntime': bun_runtime(target, directory.parent / ('bun-runtime-' + target))},
             'sourceQualification': str(directory / relative(inputs['sourceQualification'])),
             'webNotices': str(directory / relative(inputs['webNotices']))}
        for key in ('sourceQualification', 'webNotices'):
            p = Path(r[key])
            r[key] = {'path': str(p), 'sha256': sha(p), 'bytes': p.stat().st_size}
        request = directory.parent / ('js-' + target + '.json')
        need(not request.exists(), 'inspect existing JS request before retry')
        request.write_text(json.dumps(r, indent=2) + '\n')
        command = [sys.executable, str(Path(__file__).resolve().with_name('service-release.py')), 'js', '--source-root', str(checkout), '--request', str(request), '--out', str(checkout / 'build/bootstrap-js')]
    else:
        helper = Path(__file__).resolve().with_name('build-android-artifacts.py')
        command = [sys.executable, str(helper), '--execution-helper-sha256', sha(helper), '--inputs', str(directory / 'android-release-inputs.json'), '--source', str(checkout), '--output', str(checkout / 'build/bootstrap-android')]
    subprocess.run(command, cwd=checkout, check=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest='command', required=True)
    for name in ('pack', 'fetch', 'run'):
        a = sub.add_parser(name)
        a.add_argument('--directory', type=Path, required=True)
        a.add_argument('--source', required=True)
        a.add_argument('--operation', choices=OPERATIONS, required=True)
        a.add_argument('--checkout', type=Path, default=Path.cwd())
        if name == 'pack': a.add_argument('--out', type=Path, required=True)
        if name == 'fetch':
            a.add_argument('--url', required=True)
            a.add_argument('--sha256', required=True)
        if name == 'run':
            a.add_argument('--target')
    a = p.parse_args()
    a.checkout = a.checkout.resolve()
    a.directory = a.directory.resolve()
    tree = source_identity(a.source, a.checkout)
    if a.command != 'pack': record_execution(a.checkout, a.source, tree)
    if a.command == 'pack': print(json.dumps(pack(a.directory, a.out.resolve(), a.source, tree, a.operation)))
    elif a.command == 'fetch':
        fetch(a.url, a.sha256, a.directory, a.source, tree, a.operation)
        print('PASS source-bound public input packet; configured build not yet run')
    else: run(a.directory, a.operation, a.target, a.source, a.checkout)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError, tarfile.TarError):
        sys.exit('release-bootstrap: input or producer failed; inspect scoped producer receipts (no private inputs logged)')
