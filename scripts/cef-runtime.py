#!/usr/bin/env python3
"""Optional CEF payload in the existing dev/deploy package, not a deploy runner."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile

CEF_VERSION = '154.0.34'
HOST = 'backplane-cef'
HELPER = 'backplane-browser'
STAMP = HELPER + '.stamp'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def discover(prefix, cache=None):
    roots = [prefix / 'cef']
    cache = cache or Path(os.environ.get('XDG_CACHE_HOME', str(Path.home() / '.cache')))
    roots.append(cache / 'backplane' / 'cef-runtime' / CEF_VERSION)
    return next((p.resolve() for p in roots if (p / HOST).is_file()), None)


def seal(root, revision):
    files = {str(p.relative_to(root)): sha(p) for p in sorted(root.rglob('*')) if p.is_file()}
    body = {'schema': 1, 'cefVersion': CEF_VERSION, 'hostSource': revision, 'files': files}
    body['id'] = hashlib.sha256(canonical(body)).hexdigest()
    (root / 'manifest.json').write_bytes(canonical(body) + b'\n')
    return body


def verify(root):
    manifest = json.loads((root / 'manifest.json').read_text())
    identity = manifest.pop('id')
    if not re.fullmatch('[0-9a-f]{64}', identity) or hashlib.sha256(canonical(manifest)).hexdigest() != identity:
        raise ValueError('CEF manifest identity mismatch')
    if manifest['schema'] != 1 or manifest['cefVersion'] != CEF_VERSION:
        raise ValueError('unsupported CEF payload')
    files = manifest['files']
    for name in [HELPER, STAMP, 'cef/' + HOST, 'cef/libcef.so', 'cef/icudtl.dat']:
        if name not in files:
            raise ValueError('incomplete CEF payload: ' + name)
    actual = set()
    for p in root.rglob('*'):
        if p.is_symlink():
            raise ValueError('symlink in CEF payload')
        if p.is_file() and p.relative_to(root).as_posix() != 'manifest.json':
            actual.add(str(p.relative_to(root)))
    if actual != set(files):
        raise ValueError('CEF payload file set mismatch')
    for name, digest in files.items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or sha(root / name) != digest:
            raise ValueError('CEF payload hash mismatch: ' + name)
    if not os.access(root / HELPER, os.X_OK) or not os.access(root / 'cef' / HOST, os.X_OK):
        raise ValueError('CEF payload executables are not executable')
    return identity


def assemble(dest, runtime, host, helper, stamp, revision):
    if dest.exists():
        raise ValueError('CEF staging destination already exists')
    try:
        dest.mkdir(parents=True)
        shutil.copytree(runtime, dest / 'cef', symlinks=False)
        shutil.copy2(host, dest / 'cef' / HOST)
        shutil.copy2(helper, dest / HELPER)
        shutil.copy2(stamp, dest / STAMP)
        seal(dest, revision)
        verify(dest)
    except BaseException:
        shutil.rmtree(dest, ignore_errors=True)
        raise


def pointer(path, target):
    temp = path.with_name(path.name + '.new')
    if temp.exists() or temp.is_symlink():
        temp.unlink()
    temp.symlink_to(target)
    os.replace(temp, path)


def store(payload, prefix):
    identity = verify(payload)
    directory = prefix / 'browser-runtimes'
    directory.mkdir(parents=True, exist_ok=True)
    final = directory / identity
    if final.exists():
        if verify(final) != identity:
            raise ValueError('existing CEF bundle mismatch')
        return final
    temp = Path(tempfile.mkdtemp(prefix='.stage-', dir=directory))
    try:
        shutil.copytree(payload, temp, dirs_exist_ok=True)
        verify(temp)
        os.rename(temp, final)
    finally:
        shutil.rmtree(temp, ignore_errors=True)
    return final


def install(package, prefix):
    payload = package / 'browser-runtime'
    if not payload.exists():
        return
    # A package built elsewhere must not opt a new machine into CEF.
    runtime = discover(prefix)
    if runtime is None:
        return
    # Refuse incompatible aliases before making any changes to the live pair.
    for name in (HELPER, STAMP, 'cef'):
        old = prefix / name
        if old.is_dir() and not old.is_symlink():
            raise ValueError('refusing to replace a CEF directory; relocate it before deployment')
    final = store(payload, prefix)  # validate/copy before touching the current pair
    current = prefix / 'browser-current'
    previous = current.readlink() if current.is_symlink() else None
    # Retain the previously installed pair, including pre-manifest installations.
    if previous is None and (prefix / HELPER).is_file():
        snapshot = Path(tempfile.mkdtemp(prefix='.legacy-', dir=prefix)) / 'payload'
        try:
            stamp = prefix / STAMP
            if not stamp.exists():
                stamp = snapshot.parent / STAMP
                stamp.write_text('legacy\n')
            assemble(snapshot, runtime, runtime / HOST, prefix / HELPER, stamp, 'legacy')
            previous = store(snapshot, prefix).relative_to(prefix)
        finally:
            shutil.rmtree(snapshot.parent, ignore_errors=True)
    # Validate the staged ordinary helper too: no conflicting helper may replace
    # the paired symlink later in the existing fixed-file install loop.
    for name in (HELPER, STAMP):
        ordinary = package / name
        if ordinary.exists() and sha(ordinary) != sha(final / name):
            raise ValueError('staged helper differs from CEF manifest')
    if previous is not None and str(previous) != str(final.relative_to(prefix)):
        pointer(prefix / 'browser-previous', previous)
    # Prepare stable aliases before one atomic pointer promotes host + helper.
    # Existing processes keep their executable and old immutable bundle.
    if not current.exists():
        if previous is not None:
            pointer(current, previous)
    for name, target in ((HELPER, 'browser-current/' + HELPER),
                         (STAMP, 'browser-current/' + STAMP),
                         ('cef', 'browser-current/cef')):
        pointer(prefix / name, target)
    pointer(current, final.relative_to(prefix))
    for name in (HELPER, STAMP):
        (package / name).unlink(missing_ok=True)


def rollback(prefix):
    previous = prefix / 'browser-previous'
    target = previous.readlink()
    verify(prefix / target)
    current = (prefix / 'browser-current').readlink()
    pointer(prefix / 'browser-current', target)
    pointer(previous, current)


def main():
    command, *args = sys.argv[1:]
    if command == 'discover':
        found = discover(Path(args[0]))
        if found is not None:
            print(found)
    elif command == 'stage':
        package, runtime, host, helper, stamp, revision = args
        dest = Path(package) / 'browser-runtime'
        assemble(dest, Path(runtime), Path(host), Path(helper), Path(stamp), revision)
        shutil.copy2(__file__, Path(package) / 'cef-runtime.py')
    elif command == 'install':
        install(*map(Path, args))
    elif command == 'rollback':
        rollback(Path(args[0]))
    elif command == 'verify':
        print(verify(Path(args[0])))
    else:
        raise ValueError('unknown CEF payload command')


if __name__ == '__main__':
    main()
