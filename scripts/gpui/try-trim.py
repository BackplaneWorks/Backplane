#!/usr/bin/env python3
"""A smaller log for scripts/gpui/try.sh: keeps every project, the most recently active
threads whole, and only small settings. The JS hub cannot import the full store copy under
3.7 GiB. Prints counts only: the log holds real conversations."""
import json, os, sys
src, dst, keep = sys.argv[1], sys.argv[2], int(sys.argv[3])
BIG = 2048

def thread_of(e):
    k = e.get('$')
    if k == 'ThreadCreated': return e.get('id')
    return e.get('thread') if isinstance(e.get('thread'), str) else None

last = {}
with open(src, 'rb') as f:
    for i, line in enumerate(f):
        try: e = json.loads(line)
        except ValueError: continue
        t = thread_of(e)
        if t: last[t] = i
kept = set(sorted(last, key=last.get, reverse=True)[:keep])
n = dropped = big = 0
with open(src, 'rb') as f, open(dst, 'wb') as o:
    for line in f:
        try: e = json.loads(line)
        except ValueError: continue
        t = thread_of(e)
        if t and t not in kept: dropped += 1; continue
        if e.get('$') == 'SettingSet' and len(line) > BIG and not e.get('key', '').startswith(('restart.', 'person.', 'tz.')):
            big += 1; continue
        o.write(line); n += 1
os.chmod(dst, 0o600)
print(f'threads {len(last)} -> {len(kept)}; events kept {n}, dropped {dropped}, large settings dropped {big}; {os.path.getsize(dst)>>20} MB')
