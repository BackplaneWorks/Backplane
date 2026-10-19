#!/usr/bin/env python3
"""Fetch a pinned, verified KiCad demo for the explicitly selected real-CAD lane."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, default=Path('build/fixtures/ecc83'))
args = parser.parse_args()
source = json.loads(Path('test/fixtures/kicad-demo-source.json').read_text())
args.out.mkdir(parents=True, exist_ok=True)
for item in source['files']:
    url = f"https://raw.githubusercontent.com/{source['repository']}/{source['commit']}/{item['path']}"
    dest = args.out / Path(item['path']).name
    data = dest.read_bytes() if dest.exists() else b''
    if hashlib.sha256(data).hexdigest() != item['sha256']:
        data = urllib.request.urlopen(url, timeout=60).read()
    if len(data) != item['bytes'] or hashlib.sha256(data).hexdigest() != item['sha256']:
        raise SystemExit(f'fixture verification failed: {url}')
    with tempfile.NamedTemporaryFile(dir=args.out, delete=False) as temp:
        temp.write(data)
        pending = Path(temp.name)
    pending.replace(dest)
    print(f"verified {item['sha256']} {dest}")
(args.out / 'SOURCE.json').write_text(json.dumps(source, indent=2) + '\n')
print('KiCad ecc83 demo, CC BY-SA 4.0; attribution and license are beside the fixture.')
