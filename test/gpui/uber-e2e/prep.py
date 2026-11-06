#!/usr/bin/python3
"""prep.py SRC.jsonl DST.jsonl FOLDER: the fixture for a throwaway home: Bub's folder (WorktreeSet) moved off the live home. Prints counts only."""
import json, sys
n = m = 0
with open(sys.argv[1], encoding='utf-8', errors='surrogateescape') as f, open(sys.argv[2], 'w', encoding='utf-8', errors='surrogateescape') as o:
    for l in f:
        n += 1
        if l.startswith('{"$":"WorktreeSet"') or '"$":"WorktreeSet"' in l[:40]:
            e = json.loads(l); e['worktree'] = sys.argv[3]; l = json.dumps(e, separators=(',', ':')) + '\n'; m += 1
        o.write(l)
print(n, 'events,', m, 'folder moved')
