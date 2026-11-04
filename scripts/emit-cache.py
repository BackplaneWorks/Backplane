#!/usr/bin/env python3
"""Integrity of raw JS emission cache bytes; not a proof/producer attestation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

SCHEMA = 'backplane-emit-cache-sha256-v1'


class CacheMiss(ValueError):
    pass


def paths(cache, key):
    if not re.fullmatch(r'[0-9a-f]{40}(?:[0-9a-f]{24})?', key):
        raise CacheMiss('invalid emission input key')
    cache = Path(cache)
    return cache/(key+'.js'), cache/(key+'.js.ok')


def open_regular(path):
    if Path(path).is_symlink():
        raise CacheMiss('linked cache input')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0))
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        os.close(fd)
        raise CacheMiss('cache input is not a regular file')
    return fd


def stamp(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def write_all(fd, data):
    at = 0
    while at < len(data):
        count = os.write(fd, data[at:])
        if count <= 0:
            raise OSError('short cache write')
        at += count


def snapshot(source, output_fd=None, expected_size=None, strict_metadata=True):
    """Hash exactly the bounded bytes copied, keeping one input descriptor."""
    fd = open_regular(source)
    try:
        before = os.fstat(fd)
        if before.st_size <= 0 or (expected_size is not None and before.st_size != expected_size):
            raise CacheMiss('emission size mismatch')
        digest = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            block = os.read(fd, min(1024*1024, remaining))
            if not block:
                raise CacheMiss('emission changed while reading')
            remaining -= len(block)
            digest.update(block)
            if output_fd is not None:
                write_all(output_fd, block)
        after = os.fstat(fd)
        same_file = (after.st_dev, after.st_ino, after.st_size) == (before.st_dev, before.st_ino, before.st_size)
        if not same_file or (strict_metadata and stamp(after) != stamp(before)):
            raise CacheMiss('emission changed while reading')
        return digest.hexdigest(), before.st_size
    finally:
        os.close(fd)


def receipt(cache, key):
    _, path = paths(cache, key)
    fd = open_regular(path)
    try:
        before = os.fstat(fd)
        if not 0 < before.st_size <= 4096:
            raise CacheMiss('invalid emission receipt size')
        data = b''
        while len(data) < before.st_size:
            block = os.read(fd, before.st_size-len(data))
            if not block:
                raise CacheMiss('partial emission receipt')
            data += block
        if stamp(os.fstat(fd)) != stamp(before):
            raise CacheMiss('emission receipt changed')
    finally:
        os.close(fd)
    def unique(pairs):
        result = {}
        for name, value in pairs:
            if name in result:
                raise CacheMiss('duplicate emission receipt field')
            result[name] = value
        return result
    record = json.loads(data, object_pairs_hook=unique)
    if (not isinstance(record, dict) or set(record) != {'schema', 'key', 'sha256', 'bytes'}
        or record['schema'] != SCHEMA or record['key'] != key
        or type(record['bytes']) is not int or record['bytes'] <= 0
        or not isinstance(record['sha256'], str) or not re.fullmatch(r'[0-9a-f]{64}', record['sha256'])):
        raise CacheMiss('missing or incompatible emission receipt')
    return record


def verify(cache, key):
    """Warm-build hint only; a later restore independently revalidates bytes."""
    try:
        record = receipt(cache, key)
        actual = snapshot(paths(cache, key)[0], expected_size=record['bytes'], strict_metadata=False)
        return actual == (record['sha256'], record['bytes'])
    except (OSError, ValueError, RecursionError):
        return False


def restore(cache, key, output):
    temporary = None
    try:
        record = receipt(cache, key)
        output = Path(output)
        output.parent.mkdir(parents=True, exist_ok=True)
        fd, temporary = tempfile.mkstemp(prefix='.emit-copy-', dir=output.parent)
        with os.fdopen(fd, 'wb') as copied:
            # Readers may concurrently touch this file for LRU. Its exact
            # copied bytes must match the known digest; timestamps are not
            # part of that gate. Fresh publishers still require stable stats.
            actual = snapshot(paths(cache, key)[0], copied.fileno(), record['bytes'], strict_metadata=False)
        if actual != (record['sha256'], record['bytes']):
            raise CacheMiss('emission digest mismatch')
        os.replace(temporary, output)
        temporary = None
        try:
            os.utime(paths(cache, key)[0])
        except OSError:
            pass  # Read-only cache hits remain usable.
        return True
    except (OSError, ValueError, RecursionError):
        return False
    finally:
        if temporary is not None:
            Path(temporary).unlink(missing_ok=True)


def publish(cache, key, source, expected_sha256=None):
    """Publish a complete data file before its source-key-bound digest receipt."""
    data_path, receipt_path = paths(cache, key)
    data_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = record_temporary = None
    try:
        fd, temporary = tempfile.mkstemp(prefix='.emit-data-', dir=data_path.parent)
        with os.fdopen(fd, 'wb') as copied:
            digest, size = snapshot(source, copied.fileno())
        if expected_sha256 is not None and digest != expected_sha256:
            raise CacheMiss('transferred emission digest mismatch')
        record = {'schema': SCHEMA, 'key': key, 'sha256': digest, 'bytes': size}
        os.replace(temporary, data_path)
        temporary = None
        fd, record_temporary = tempfile.mkstemp(prefix='.emit-receipt-', dir=data_path.parent)
        with os.fdopen(fd, 'wb') as saved:
            saved.write((json.dumps(record, sort_keys=True)+'\n').encode())
        os.replace(record_temporary, receipt_path)
        record_temporary = None
    finally:
        for path in (temporary, record_temporary):
            if path is not None:
                Path(path).unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('restore', 'publish', 'verify'))
    parser.add_argument('--cache', required=True)
    parser.add_argument('--key', required=True)
    parser.add_argument('--file')
    args = parser.parse_args()
    if args.operation != 'verify' and not args.file:
        parser.error('--file is required for restore/publish')
    if args.operation == 'restore':
        return 0 if restore(args.cache, args.key, args.file) else 1
    if args.operation == 'verify':
        return 0 if verify(args.cache, args.key) else 1
    try:
        publish(args.cache, args.key, args.file)
    except (OSError, ValueError) as error:
        print('emit-cache: '+str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
