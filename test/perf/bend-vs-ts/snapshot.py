"""Copy a bounded real log locally. Never write to its source or start a hub.

The raw copy remains under ignored build/. Only schema/counts are printed.
"""
import collections
import hashlib
import json
import os
import sys
from pathlib import Path

src, dest = map(Path, sys.argv[1:3])
dest.mkdir(parents=True, exist_ok=True)
digest = hashlib.sha256()
counts = collections.Counter()
schemas = {}
threads = set()
with src.open('rb') as source, (dest / 'private-events.jsonl').open('wb') as target:
    os.chmod(target.name, 0o600)
    for n, line in enumerate(source, 1):
        if n > 1_450_000:
            break
        if not line.endswith(b'\n'):
            break
        item = json.loads(line)
        target.write(line)
        digest.update(line)
        tag = item['$']
        counts[tag] += 1
        schemas[tag] = {k: type(v).__name__ for k, v in item.items()}
        if tag == 'ThreadCreated':
            threads.add(item['id'])
meta = dict(events=sum(counts.values()), sha256=digest.hexdigest(),
            bytes=(dest / 'private-events.jsonl').stat().st_size,
            counts=dict(counts), schemas=schemas, threads=len(threads))
(dest / 'snapshot.json').write_text(json.dumps(meta, indent=2) + '\n')
print(json.dumps({k: v for k, v in meta.items() if k != 'schemas'}))
