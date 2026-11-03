#!/usr/bin/env python3
"""Offline archive IO for the Electron producer. Never executes a payload."""
import datetime
import gzip
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import sys
import tarfile
import zipfile


def member(name):
    p = PurePosixPath(name)
    if not name or '\\' in name or name.startswith('/') or any(x in ('', '.', '..') for x in name.split('/')) or ':' in name or '\x00' in name:
        raise ValueError('Unsafe archive member')
    return p


def safe_link(name, target):
    if not target or '\\' in target or target.startswith('/') or ':' in target:
        raise ValueError('Unsafe archive link')
    parts = list(PurePosixPath(name).parent.parts)
    for part in target.split('/'):
        if part == '..':
            if not parts:
                raise ValueError('Escaping archive link')
            parts.pop()
        elif part not in ('', '.'):
            parts.append(part)


def extract(archive, output):
    output.mkdir()
    seen = set()
    pending_links = []
    total_bytes = 0
    def destination(name):
        name = name.rstrip('/')
        member(name)
        if name.casefold() in seen:
            raise ValueError('Duplicate/case-colliding archive member')
        seen.add(name.casefold())
        if len(seen) > 200000:
            raise ValueError('Archive member limit')
        return output.joinpath(*PurePosixPath(name).parts), name
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as z:
            for info in z.infolist():
                dest, name = destination(info.filename)
                mode = info.external_attr >> 16
                total_bytes += info.file_size
                if info.file_size > 8 * 1024**3 or total_bytes > 16 * 1024**3 or mode & 0o7000:
                    raise ValueError('Archive size/privileged-mode limit')
                if info.external_attr & 0x400:
                    raise ValueError('Windows reparse archive member')
                if info.is_dir():
                    dest.mkdir(parents=True, exist_ok=True)
                    continue
                if stat.S_ISLNK(mode):
                    target = z.read(info).decode('utf8')
                    safe_link(name, target)
                    pending_links.append((dest, target))
                elif stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                    raise ValueError('Special archive member')
                else:
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with z.open(info) as source, dest.open('xb') as sink:
                        shutil.copyfileobj(source, sink)
                    os.chmod(dest, (mode & 0o777) or 0o644)
    else:
        with tarfile.open(archive, 'r:*') as t:
            for info in t:
                dest, name = destination(info.name)
                total_bytes += info.size
                if info.size > 8 * 1024**3 or total_bytes > 16 * 1024**3 or info.mode & 0o7000:
                    raise ValueError('Archive size/privileged-mode limit')
                if info.isdir():
                    dest.mkdir(parents=True, exist_ok=True)
                elif info.issym():
                    safe_link(name, info.linkname)
                    pending_links.append((dest, info.linkname))
                elif info.isfile():
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with t.extractfile(info) as source, dest.open('xb') as sink:
                        shutil.copyfileobj(source, sink)
                    os.chmod(dest, info.mode & 0o777)
                else:
                    raise ValueError('Hardlink/device/special archive member')
    # No file is ever written through a symlink during extraction.
    names = {str(p.relative_to(output)).replace(os.sep, '/') for p in output.rglob('*')}
    names.update(str(p.relative_to(output)).replace(os.sep, '/') for p, _ in pending_links)
    for dest, target in pending_links:
        rel = str(dest.relative_to(output)).replace(os.sep, '/')
        if any(n.startswith(rel + '/') for n in names):
            raise ValueError('Archive writes below a link')
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.symlink_to(target)


def archive(root, output, prefix, epoch):
    paths = sorted(root.rglob('*'), key=lambda p: p.relative_to(root).as_posix())
    if output.suffix == '.zip':
        timestamp = datetime.datetime.fromtimestamp(max(epoch, 315532800), datetime.timezone.utc)
        with zipfile.ZipFile(output, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
            for p in paths:
                if p.is_symlink():
                    raise ValueError('Windows ZIP cannot contain symlinks')
                if not p.is_file():
                    continue
                info = zipfile.ZipInfo(prefix + p.relative_to(root).as_posix(), timestamp.timetuple()[:6])
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | (p.stat().st_mode & 0o777)) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, p.read_bytes(), compresslevel=9)
    else:
        with output.open('xb') as sink, gzip.GzipFile(filename='', fileobj=sink, mode='wb', mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as t:
            for p in paths:
                info = t.gettarinfo(str(p), prefix + p.relative_to(root).as_posix())
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                info.mtime = epoch
                if info.isfile():
                    with p.open('rb') as source:
                        t.addfile(info, source)
                else:
                    t.addfile(info)


def plist(filename, identifier, version):
    import plistlib
    p = Path(filename)
    data = plistlib.loads(p.read_bytes())
    data.update(CFBundleExecutable='Backplane', CFBundleIdentifier=identifier,
                CFBundleName='Backplane', CFBundleDisplayName='Backplane',
                CFBundleVersion=version, CFBundleShortVersionString=version)
    data['CFBundleURLTypes'] = [{'CFBundleURLName': 'Backplane pairing', 'CFBundleURLSchemes': ['backplane']}]
    p.write_bytes(plistlib.dumps(data, fmt=plistlib.FMT_XML, sort_keys=True))


if __name__ == '__main__':
    op, *args = sys.argv[1:]
    if op == 'extract':
        extract(Path(args[0]), Path(args[1]))
    elif op == 'archive':
        archive(Path(args[0]), Path(args[1]), args[2], int(args[3]))
    elif op == 'plist-icon':
        import plistlib
        p = Path(args[0])
        data = plistlib.loads(p.read_bytes())
        data['CFBundleIconFile'] = args[1]
        p.write_bytes(plistlib.dumps(data))
    elif op == 'plist':
        plist(*args)
    else:
        raise ValueError('Unknown offline archive operation')
