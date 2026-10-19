#!/usr/bin/env python3
"""Read archive entries and bytes without executing/extracting their payloads."""
import hashlib
import json
import os
from pathlib import PurePosixPath
import re
import subprocess
import sys
import tarfile

LIMIT = 1024 * 1024 * 1024


def path_name(name):
    while name.startswith('./'):
        name = name[2:]
    name = name.rstrip('/')
    if name == '.':
        name = ''
    parts = PurePosixPath(name).parts
    if name.startswith('/') or '..' in parts or '\x00' in name or '\n' in name:
        raise ValueError('unsafe archive member path')
    if name and str(PurePosixPath(name)) != name:
        raise ValueError('noncanonical archive member path')
    return name


def digest(stream, size):
    if not 0 <= size <= LIMIT:
        raise ValueError('archive payload exceeds read bound')
    result = hashlib.sha256()
    total = 0
    while chunk := stream.read(1024 * 1024):
        total += len(chunk)
        if total > size:
            raise ValueError('archive payload exceeds declared size')
        result.update(chunk)
    if total != size:
        raise ValueError('archive payload is truncated')
    return result.hexdigest()


def tar_entries(path):
    source = sys.stdin.buffer if path == '-' else open(path, 'rb')
    entries = []
    with tarfile.open(fileobj=source, mode='r|*') as archive:
        for member in archive:
            if not (member.isdir() or member.isfile()) or member.mode & 0o7000:
                raise ValueError('archive payload must contain regular files/directories, without special mode bits')
            entry = {'path': path_name(member.name), 'kind': 'directory' if member.isdir() else 'file',
                     'access': member.mode & 0o555}
            if member.isfile():
                stream = archive.extractfile(member)
                entry.update(size=member.size, sha256=digest(stream, member.size))
            entries.append(entry)
    return entries


def squash_entries(path, offset):
    env = {k: v for k, v in os.environ.items() if k != 'BACKPLANE_PERF_SIGNING_KEY'}
    listing = subprocess.run(['unsquashfs', '-lln', '-processors', '1', '-o', offset, path],
                             check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, text=True)
    entries = []
    pattern = re.compile(r'^([d-][rwx-]{9})\s+\d+/\d+\s+(\d+)\s+\d{4}-\d\d-\d\d\s+\d\d:\d\d\s+(squashfs-root(?:/.*)?)$')
    for line in listing.stdout.splitlines():
        if not line:
            continue
        match = pattern.fullmatch(line)
        if not match:
            raise ValueError('unsupported SquashFS entry/type/mode')
        permissions, size, name = match.groups()
        name = path_name(name.removeprefix('squashfs-root').removeprefix('/'))
        mode = sum(bit for position, bit in [(1, 0o400), (3, 0o100), (4, 0o040), (6, 0o010), (7, 0o004), (9, 0o001)]
                   if permissions[position] != '-')
        entry = {'path': name, 'kind': 'directory' if permissions[0] == 'd' else 'file', 'access': mode}
        if permissions[0] == '-':
            command = ['unsquashfs', '-cat', '-no-wildcards', '-processors', '1', '-o', offset, path, name]
            child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env)
            try:
                entry.update(size=int(size), sha256=digest(child.stdout, int(size)))
                if child.wait() != 0:
                    raise ValueError('cannot read SquashFS member')
            finally:
                if child.poll() is None:
                    child.kill()
                child.wait()
        entries.append(entry)
    return entries


def main():
    if len(sys.argv) == 3 and sys.argv[1] == 'tar':
        entries = tar_entries(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == 'squash':
        entries = squash_entries(sys.argv[2], sys.argv[3])
    else:
        raise ValueError('usage: perf-payload.py tar FILE|- | squash FILE OFFSET')
    if len(entries) > 100000 or len({e['path'] for e in entries}) != len(entries):
        raise ValueError('duplicate or excessive archive entries')
    print(json.dumps(sorted(entries, key=lambda e: e['path'])))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'archive payload REJECTED: {error}', file=sys.stderr)
        sys.exit(1)
