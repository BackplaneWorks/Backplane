#!/usr/bin/env python3
"""Send an explicit Bend input closure through the existing budgeted Boat runner.

This is transport, not a gate cache: it never manufactures or reuses a pass.
The remote verifies the packet, compiler and libraries before AND after the
command. Existing Boat admission, budgets, cleanup and exit codes still apply.
"""
import argparse
import fnmatch
import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time

SELF = 'scripts/ci-packet.py'
CLOSURE = 'scripts/bend-closure.py'
PIN = '.github/actions/setup-bend/action.yml'
ENV = {'BEND_NO_TELEMETRY', 'BUN_JSC_forceRAMSize', 'BUN_JSC_maxPerThreadStackUsage'}
DENY = ['.env', '.env.*', '*.pem', '*.key', '*.p12', '*.pfx', 'id_rsa*',
        'id_ed25519*', 'id_ecdsa*', 'auth.json', 'credentials*', '*.keystore', '*.jks', '*.log']


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        while data := f.read(1024 * 1024):
            h.update(data)
    return h.hexdigest()


def safe_path(name):
    p = Path(name)
    if (not name or p.is_absolute() or p.as_posix() != name or
            any(part in ('', '.', '..', '.git') for part in name.split('/')) or
            any(ord(c) < 32 for c in name)):
        raise ValueError('unsafe packet path: ' + repr(name))
    return p


def source(path):
    p = safe_path(path)
    if (any(fnmatch.fnmatch(p.name, pattern) for pattern in DENY) or
            any(part in {'node_modules', '.claude', '.backplane', '.backplane-bend', 'shots-snap'} for part in p.parts) or
            (p.suffix == '.jsonl' and not path.startswith('test/'))):
        raise ValueError('packet refuses private/generated capture input: ' + path)
    if any(parent.is_symlink() for parent in [p, *p.parents]):
        raise ValueError('packet refuses symbolic links: ' + path)
    if not p.is_file():
        raise ValueError('packet input is not a regular file: ' + path)
    mode = stat.S_IMODE(p.stat().st_mode)
    if mode & 0o7000:
        raise ValueError('packet refuses special permission bits: ' + path)
    return {'sha256': digest(p), 'mode': mode}


def resources(folder):
    if not folder.exists():
        return {}
    if folder.is_symlink() or not folder.is_dir():
        raise ValueError('tool resource folder is not a directory: ' + str(folder))
    out = {}
    for p in sorted(folder.rglob('*')):
        if p.is_symlink():
            raise ValueError('tool resources contain a symbolic link: ' + str(p))
        if p.is_file():
            out[p.relative_to(folder).as_posix()] = digest(p)
    return out


def toolchain():
    found = shutil.which('bend')
    if not found:
        raise ValueError('bend is not on PATH')
    binary = Path(found).resolve(strict=True)
    bundled = binary.parent.parent / 'bend2'
    if not (bundled / 'base.bend').is_file():
        raise ValueError('cannot identify Bend bundled libraries')
    return {'binary_sha256': digest(binary), 'bundled': resources(bundled),
            'named_libraries': resources(Path(os.environ.get('BEND_LIB', Path.home()/'.bend/lib')))}


def inventory():
    data = subprocess.check_output(['git', 'ls-files', '-co', '--exclude-standard', '-z'])
    return set(data.decode().rstrip('\0').split('\0')) - {''}


def create(roots, extras, manifest, environment):
    safe_path(manifest)
    closure = subprocess.check_output([sys.executable, CLOSURE, *roots], text=True).splitlines()
    files = set(closure) | set(extras) | {SELF, CLOSURE, PIN, '.gitignore'}
    if manifest in files:
        raise ValueError('manifest cannot be one of its own inputs')
    # Generated/ignored Bend roots are allowed only by this explicit closure.
    # They are packed directly, never found by walking an ignored directory.
    if not roots or any(not r.endswith('.bend') for r in roots):
        raise ValueError('packet needs explicit Bend roots')
    env = {}
    for item in environment:
        name, sep, value = item.partition('=')
        if not sep or name not in ENV:
            raise ValueError('unsupported packet environment: ' + item)
        env[name] = value
    data = {'schema': 1, 'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
            'roots': sorted(roots), 'files': {f: source(f) for f in sorted(files)},
            'toolchain': toolchain(), 'environment': env}
    target = Path(manifest)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')
    print(json.dumps({'manifest': manifest, 'sha256': digest(target), 'files': len(files),
                      'source_bytes': sum(Path(f).stat().st_size for f in files)}), flush=True)


def verify(manifest, expected, complete=True, transport=None):
    safe_path(manifest)
    if digest(manifest) != expected:
        raise ValueError('packet manifest hash mismatch')
    data = json.loads(Path(manifest).read_text())
    if data.get('schema') != 1 or not data.get('roots') or not data.get('files'):
        raise ValueError('unsupported or empty packet manifest')
    if not set(data.get('environment', {})) <= ENV:
        raise ValueError('unsupported packet environment')
    for f, before in data['files'].items():
        if source(f) != before:
            raise ValueError('packet input changed: ' + f)
    if toolchain() != data['toolchain']:
        raise ValueError('packet compiler/library identity mismatch')
    for path, checksum in (transport or {}).items():
        safe_path(path)
        if digest(path) != checksum:
            raise ValueError('packet transport archive changed')
    if complete:
        # Explicit generated roots can be ignored by Git. Walk the received
        # packet instead; only .git metadata and the declared output area are
        # exempt. Git's ignore rules cannot hide unexpected source here.
        actual = set()
        for folder, dirs, files in os.walk('.'):
            if folder == '.':
                dirs[:] = [d for d in dirs if d not in {'.git', 'build'}]
            for name in [*dirs, *files]:
                path = Path(folder)/name
                if path.is_symlink():
                    raise ValueError('packet contains a symbolic link')
            actual.update((Path(folder)/name).as_posix() for name in files)
        # The manifest may accompany explicitly declared generated roots under
        # build/. Its digest and every declared input were verified above, just
        # like build/ source files; the output-area walk omits both consistently.
        expected_files = {f for f in {*data['files'], manifest, *(transport or {})}
                          if not f.startswith('build/')}
        if actual != expected_files:
            raise ValueError('packet contains missing/unexpected source: ' +
                             repr(sorted(actual ^ expected_files)[:8]))
    return data


def archive_members(archive, manifest, expected):
    """Validate every payload byte before extracting or accepting a cached archive."""
    with tarfile.open(archive, 'r:gz') as t:
        members = t.getmembers()
        names = [m.name for m in members]
        if len(names) != len(set(names)) or any(not m.isfile() for m in members):
            raise ValueError('packet archive contains duplicate/non-regular entries')
        for name in names:
            safe_path(name)
        if manifest not in names:
            raise ValueError('packet archive lacks its manifest')
        body = t.extractfile(manifest).read()
        if hashlib.sha256(body).hexdigest() != expected:
            raise ValueError('packet archive manifest hash mismatch')
        data = json.loads(body)
        if set(names) != set(data['files']) | {manifest}:
            raise ValueError('packet archive file set mismatch')
        payload = {manifest: body}
        for m in members:
            if m.name == manifest:
                continue
            content = t.extractfile(m).read()
            before = data['files'][m.name]
            if hashlib.sha256(content).hexdigest() != before['sha256'] or m.mode != before['mode']:
                raise ValueError('packet archive input mismatch: ' + m.name)
            payload[m.name] = content
        return data, payload


def bundle(manifest, expected):
    data = verify(manifest, expected, complete=False)
    folder = Path('.ci-packet')
    folder.mkdir(exist_ok=True)
    target = folder/(expected+'.tar.gz')
    hit = False
    if target.exists():
        try:
            archive_members(target, manifest, expected)
            hit = True
        except (ValueError, OSError, EOFError, tarfile.TarError):
            pass  # Repair a corrupt transport cache; never accept it as a pass.
    if not hit:
        with tempfile.NamedTemporaryFile(dir=folder, delete=False) as f:
            temp = Path(f.name)
        try:
            with temp.open('wb') as out, gzip.GzipFile(fileobj=out, mode='wb', mtime=0, filename='', compresslevel=1) as gz, tarfile.open(fileobj=gz, mode='w') as t:
                for name in sorted(set(data['files']) | {manifest}):
                    body = Path(name).read_bytes()
                    m = tarfile.TarInfo(name)
                    m.size = len(body)
                    m.mode = data['files'][name]['mode'] if name in data['files'] else 0o644
                    t.addfile(m, io.BytesIO(body))
            archive_members(temp, manifest, expected)
            verify(manifest, expected, complete=False)
            temp.replace(target)
        finally:
            temp.unlink(missing_ok=True)
    print(json.dumps({'payload': str(target), 'payload_sha256': digest(target),
                      'payload_bytes': target.stat().st_size, 'transport_cache_hit': hit}), flush=True)
    return target


def unpack(archive, checksum, manifest, expected):
    safe_path(archive)
    if digest(archive) != checksum:
        raise ValueError('packet transport archive hash mismatch')
    data, payload = archive_members(archive, manifest, expected)
    # No extractall: traversal, links, duplicates, undeclared and changed bytes
    # were rejected above. Only regular files in the declared closure are made.
    for name, body in payload.items():
        target = safe_path(name)
        if any(p.is_symlink() for p in [target, *target.parents]):
            raise ValueError('packet extraction refuses symbolic links')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(body)
        target.chmod(data['files'][name]['mode'] if name in data['files'] else 0o644)


def literal_pattern(path):
    # boat-job's --exclude takes fnmatch patterns. Quote metacharacters so
    # an actual filename cannot accidentally retain/exclude another input.
    return ''.join({'[': '[[]', '*': '[*]', '?': '[?]'}.get(c, c) for c in path)


def run(manifest, expected, command, timeout, size):
    data = verify(manifest, expected, complete=False)
    archive = bundle(manifest, expected)
    checksum = digest(archive)
    # A directory push performs many remote file operations. One compressed,
    # content-addressed archive plus the verifier and pin avoids that latency.
    kept = {SELF, PIN, str(archive)}
    known = inventory()
    if not kept <= known:
        raise ValueError('transport bootstrap/archive is ignored or missing; no Boat job started')
    argv = ['scripts/boat-job.sh', '--size', size, '--timeout', str(timeout),
            '--wait', '0', '--idle', '0', '--artifact', 'build/ci-packet']
    for f in sorted(known - kept):
        argv += ['--exclude', literal_pattern(f)]
    argv += ['--', 'python3', SELF, 'unpack-execute', '--manifest', manifest,
             '--expect', expected, '--archive', str(archive), '--archive-sha256', checksum, '--', *command]
    print(f'packet {expected}: transferring {len(kept)} files for {len(data["files"])} source inputs; identities verified remotely', flush=True)
    # No second controller, credentials, shell interpolation or fallback job.
    os.execvp(argv[0], argv)


def execute(manifest, expected, command, transport=None):
    out = Path('build/ci-packet')
    out.mkdir(parents=True, exist_ok=True)
    (out/'result.json').unlink(missing_ok=True)
    data = verify(manifest, expected, transport=transport)
    env = {**os.environ, **data['environment']}
    start = time.monotonic()
    result = subprocess.run(command, env=env)
    # A command changing its inputs cannot leave a successful receipt.
    verify(manifest, expected, transport=transport)
    record = {'packet_sha256': expected, 'revision': data['revision'],
              'command': command, 'seconds': round(time.monotonic()-start, 3),
              'returncode': result.returncode, 'inputs_unchanged': True,
              'source_files': len(data['files']), 'transport': transport or {}, 'toolchain_sha256': hashlib.sha256(
                  json.dumps(data['toolchain'], sort_keys=True).encode()).hexdigest()}
    (out/'result.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(record), flush=True)
    return result.returncode if result.returncode >= 0 else 128-result.returncode


def main():
    p = argparse.ArgumentParser(description=__doc__)
    modes = p.add_subparsers(dest='mode', required=True)
    c = modes.add_parser('create')
    c.add_argument('--root', action='append', required=True)
    c.add_argument('--extra', action='append', default=[])
    c.add_argument('--env', action='append', default=[])
    c.add_argument('--manifest', required=True)
    for mode in ['verify', 'bundle', 'run', 'execute', 'unpack-execute']:
        q = modes.add_parser(mode)
        q.add_argument('--manifest', required=True)
        q.add_argument('--expect', required=True)
        if mode == 'run':
            q.add_argument('--timeout', type=int, default=180)
            q.add_argument('--size', choices=['small', 'default', 'large'], default='default')
        if mode == 'unpack-execute':
            q.add_argument('--archive', required=True)
            q.add_argument('--archive-sha256', required=True)
        if mode not in ['verify', 'bundle']:
            q.add_argument('command', nargs=argparse.REMAINDER)
    a = p.parse_args()
    try:
        if a.mode == 'create':
            create(a.root, a.extra, a.manifest, a.env)
        elif a.mode == 'verify':
            verify(a.manifest, a.expect)
            print('ok packet source and tool identities')
        elif a.mode == 'bundle':
            bundle(a.manifest, a.expect)
        else:
            command = a.command[1:] if a.command[:1] == ['--'] else a.command
            if not command:
                raise ValueError('packet command is empty')
            if a.mode == 'run':
                if not 1 <= a.timeout <= 3600:
                    raise ValueError('timeout must be 1..3600 seconds')
                run(a.manifest, a.expect, command, a.timeout, a.size)
            elif a.mode == 'unpack-execute':
                unpack(a.archive, a.archive_sha256, a.manifest, a.expect)
                return execute(a.manifest, a.expect, command, {a.archive: a.archive_sha256})
            else:
                return execute(a.manifest, a.expect, command)
    except (ValueError, OSError, EOFError, tarfile.TarError, subprocess.CalledProcessError) as error:
        print('ci-packet: ' + str(error), file=sys.stderr)
        return 3
    return 0


if __name__ == '__main__':
    sys.exit(main())
