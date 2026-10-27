#!/usr/bin/env python3
"""Pair a dedicated simulator through the debug startup hook without logging its link."""
import json, os, re, stat, subprocess, sys
from pathlib import Path
if len(sys.argv) != 2 or not re.fullmatch(r'[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}', sys.argv[1]):
    sys.exit('usage: private-pair-launch.py DEDICATED_SIMULATOR_UDID')
udid = sys.argv[1].upper()
ssh = ['ssh', '-F', '/dev/null', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=6', '-o', 'IdentityAgent=none', '-o', 'IdentitiesOnly=yes', '-i', str(Path.home()/'.ssh/rsyncnet_ed25519'), 'h@hs-mac-mini']
listing = json.loads(subprocess.check_output(ssh + ['export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl list devices available --json']))
devices = [d for group in listing['devices'].values() for d in group if d['udid'].upper() == udid]
if len(devices) != 1 or not devices[0]['name'].startswith('Backplane qualification 655983 ') or devices[0]['state'] != 'Booted':
    sys.exit('refusing: simulator is not the booted dedicated qualification device')
path = Path.home()/'.backplane-bend/token'; info=path.stat()
if info.st_uid != os.getuid() or not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077: sys.exit('unsafe token-file owner or mode')
token=path.read_text().strip()
if not re.fullmatch(r'[0-9a-fA-F]{32}', token): sys.exit('unexpected hub token format')
link=('http://desktop.tail9e0cf2.ts.net:3787/#token='+token).encode()
# The validated UUID, but never the link, appears in the SSH command or argv.
remote = "import os,sys,subprocess; link=sys.stdin.buffer.read().decode(); e=os.environ.copy(); e['DEVELOPER_DIR']='/Applications/Xcode.app/Contents/Developer'; e['SIMCTL_CHILD_BACKPLANE_PAIR_LINK']=link; e['SIMCTL_CHILD_BACKPLANE_QUALIFICATION']='1'; sys.exit(subprocess.run(['/usr/bin/xcrun','simctl','launch','--terminate-running-process',sys.argv[1],'dev.backplane.mobile'],env=e,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode)"
command = "python3 -c " + __import__('shlex').quote(remote) + " " + udid
subprocess.run(ssh+[command],input=link,check=True)
print('Private pairing launch submitted to the dedicated simulator; verify fresh content and persistence separately.')
