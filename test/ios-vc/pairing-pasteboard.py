#!/usr/bin/env python3
"""Place the local hub pairing link in one dedicated simulator's pasteboard.

Does not print the link or put it in process arguments. The caller must paste
and connect in the simulator UI, then clear the pasteboard through stdin.
"""
import os, re, stat, subprocess, sys
from pathlib import Path

if len(sys.argv) != 2 or not re.fullmatch(r'[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}', sys.argv[1]):
    sys.exit('usage: pairing-pasteboard.py DEDICATED_SIMULATOR_UDID')
udid = sys.argv[1].upper()
identity = str(Path.home() / '.ssh/rsyncnet_ed25519.pub')
ssh = ['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=6', '-o', 'IdentitiesOnly=yes', '-i', identity, 'h@hs-mac-mini']
# Restrict this operation to the simulator created by the qualification script.
remote_check = "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl list devices available --json"
import json
listing = json.loads(subprocess.check_output(ssh + [remote_check]))
devices = [d for group in listing['devices'].values() for d in group if d['udid'].upper() == udid]
if len(devices) != 1 or not devices[0]['name'].startswith('Backplane qualification 655983 ') or devices[0]['state'] != 'Booted':
    sys.exit('refusing: simulator is not the booted dedicated qualification device')
path = Path.home() / '.backplane-bend/token'
info = path.stat()
if info.st_uid != os.getuid() or not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077:
    sys.exit('refusing: hub token file owner or permissions are unsafe')
token = path.read_text().strip()
if not re.fullmatch(r'[0-9a-fA-F]{32}', token): sys.exit('refusing: unexpected hub token format')
link = ('http://desktop.tail9e0cf2.ts.net:3787/#token=' + token).encode()
# UUID is validated; only the UUID appears in the command. The link flows on stdin.
command = f'export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl pbcopy {udid}'
subprocess.run(ssh + [command], input=link, check=True)
print('Pairing link placed on the dedicated simulator pasteboard; paste through the app UI.')
