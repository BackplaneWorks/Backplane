#!/usr/bin/env python3
"""Run one UI test and deliver its nonsecret client-only URL at its marker.

Run on a Mac from the isolated iOS source copy under the ios-vc-mac lock.
Result bundles and logs may contain private project content; keep them private.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

if len(sys.argv) != 3 or sys.argv[2] not in ('disconnect', 'reconnect'):
    sys.exit('usage: client-interruption-driver.py SIMULATOR_UDID disconnect|reconnect')
udid, operation = sys.argv[1:]
tests = {
    'disconnect': ('testOfflineViewerClose', 'QUALIFICATION_PARK_NOW'),
    'reconnect': ('testExplicitReconnectAfterOfflineClose', 'QUALIFICATION_RECONNECT_NOW'),
}
test, marker = tests[operation]
listing = json.loads(subprocess.check_output(['xcrun', 'simctl', 'list', 'devices', '--json']))
devices = [d for group in listing['devices'].values() for d in group if d['udid'] == udid]
if len(devices) != 1 or not devices[0]['name'].startswith('Backplane qualification 655983'):
    sys.exit('refusing to use a non-dedicated simulator')
result = Path('evidence') / (test + '.xcresult')
log = Path('evidence') / (test + '.log')
if result.exists() or log.exists():
    sys.exit('refusing to overwrite existing evidence')
result.parent.mkdir(mode=0o700, exist_ok=True)
cmd = ['xcodebuild', '-project', 'Backplane.xcodeproj', '-scheme', 'Qualification',
       '-destination', f'platform=iOS Simulator,id={udid}', '-derivedDataPath', 'build/dd',
       '-collect-test-diagnostics', 'never', '-parallel-testing-enabled', 'NO',
       '-resultBundlePath', str(result), f'-only-testing:QualificationUITests/QualificationUITests/{test}', 'test']
issued = False
with os.fdopen(os.open(log, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w') as output:
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
    for line in proc.stdout:
        output.write(line)
        output.flush()
        if marker in line and not issued:
            issued = True
            sent = subprocess.run(['xcrun', 'simctl', 'openurl', udid,
                                   f'backplane://qualification?op={operation}'],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            print('qualification URL exit:', sent.returncode, flush=True)
        if any(item in line for item in ('safe-offline-close', 'Test Case ', 'TEST SUCCEEDED', 'TEST FAILED')):
            print(line.rstrip(), flush=True)
    code = proc.wait()
print('qualification URL issued:', issued)
sys.exit(code if issued else 1)
