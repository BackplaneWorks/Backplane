#!/usr/bin/env python3
"""Verify/copy an explicitly attested raw emission without invoking Bend.

service-release.py establishes producer provenance before passing these three
values. This consumer binds the exact bytes to the current import/tool key,
including after memory admission. A mismatch never falls back to compilation.
"""
import argparse
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import tempfile


def consume(source, expected, key, actual, output=None):
    if not re.fullmatch(r'[0-9a-f]{64}', expected or ''):
        raise ValueError('missing/invalid retained emission SHA256')
    if not re.fullmatch(r'[0-9a-f]{40}(?:[0-9a-f]{24})?', key or '') or key != actual:
        raise ValueError('retained emission import/tool identity mismatch')
    source = Path(source).absolute()
    if any(p.is_symlink() for p in [source, *source.parents]):
        raise ValueError('retained emission symlink refused')
    fd = os.open(source, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0))
    temporary = None
    try:
        with os.fdopen(fd, 'rb') as src:
            before = os.fstat(src.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_size == 0:
                raise ValueError('retained emission must be a nonempty regular file')
            if output:
                output = Path(output)
                output.parent.mkdir(parents=True, exist_ok=True)
                raw, temporary = tempfile.mkstemp(prefix='.retained-', dir=output.parent)
                dest = os.fdopen(raw, 'wb')
            else:
                dest = None
            digest = hashlib.sha256()
            try:
                while block := src.read(1024 * 1024):
                    digest.update(block)
                    if dest:
                        dest.write(block)
            finally:
                if dest:
                    dest.close()
            after = os.fstat(src.fileno())
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise ValueError('retained emission changed while reading')
            if digest.hexdigest() != expected:
                raise ValueError('retained emission bytes mismatch')
        if temporary:
            os.replace(temporary, output)
            temporary = None
    finally:
        if temporary:
            Path(temporary).unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--actual-key', required=True)
    parser.add_argument('--output')
    args = parser.parse_args()
    try:
        consume(os.environ.get('BACKPLANE_PREEMITTED_JS', ''),
                os.environ.get('BACKPLANE_PREEMITTED_SHA256', ''),
                os.environ.get('BACKPLANE_PREEMITTED_KEY', ''), args.actual_key, args.output)
    except (OSError, ValueError) as error:
        print('retained-js: ' + str(error), file=sys.stderr)
        return 3
    return 0


if __name__ == '__main__':
    sys.exit(main())
