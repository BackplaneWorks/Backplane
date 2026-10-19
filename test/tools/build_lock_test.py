#!/usr/bin/env python3
"""An ancestor mentioning flock is not an ancestor holding flock."""
import pathlib
import subprocess
import tempfile

source = pathlib.Path('scripts/build-app.sh').read_text()
helper = source[source.index('held() {'):source.index('\nif [ -z "${BP_BUILD_LOCKED')]
with tempfile.TemporaryDirectory(prefix='bp-build-lock-test-') as tmp:
    # Exercise the real ancestry parser against a private lock so the
    # regression never waits for or interferes with an actual native build.
    lock = str(pathlib.Path(tmp) / 'bp-wt-build.lock')
    helper = helper.replace('/tmp/bp-wt-build.lock', lock)
    probe = pathlib.Path(tmp) / 'probe.sh'
    probe.write_text('#!/bin/sh\n' + helper + '\nif held; then exit 0; else exit 1; fi\n')
    # Actual shell ancestry, including misleading literal script text.
    r = subprocess.run(['sh', '-c', 'sh "$1"; result=$?; : "flock $2"; exit "$result"', 'sh', str(probe), lock])
    assert r.returncode == 1, 'script text falsely claims the native lock'
    print('ok mention in ancestor script does not bypass native lock')
    r = subprocess.run(['flock', lock, 'sh', str(probe)])
    assert r.returncode == 0, 'actual outer flock not recognized'
    print('ok actual outer lock is recognized without nested deadlock')
