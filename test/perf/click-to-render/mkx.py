#!/usr/bin/env python3
# mkx.py SRC DST [FOLD_IDS] : the X (huge) fixture: the real-data copy's log x3, interleaved in time order (48 projects, ~440 threads),
# and the two longest threads' copies folded into the originals (3x their events: ~33k / ~23k events, ~3x messages).
# ids get a per-copy counter offset (+10M / +20M); titles of copies get " ·2" / " ·3".
import json, re, sys
src, dst = sys.argv[1], sys.argv[2]
FOLD = set(sys.argv[3].split(',')) if len(sys.argv) > 3 else set()  # thread ids whose copies fold into the original
idre = re.compile(r'\b([ptcgqkbwmd])(17\d{8})-(\d+)\b')
msgre = re.compile(r'\b((?:msg|toolu|call|item|fc)_[A-Za-z0-9]+)')
out = open(dst, 'w')
n = 0
for line in open(src):
    out.write(line); n += 1
    e = json.loads(line)
    for c in (1, 2):
        def rid(m):
            full = m.group(0)
            if full in FOLD: return full
            return f'{m.group(1)}{m.group(2)}-{int(m.group(3)) + c * 10_000_000}'
        l2 = idre.sub(rid, line)
        l2 = msgre.sub(lambda m: m.group(1) + 'x' + str(c), l2)
        if e.get('$') in ('ProjectCreated', 'ThreadCreated', 'ThreadRenamed') and 'title' in e:
            if e.get('id') in FOLD: continue
            d = json.loads(l2); d['title'] = d['title'] + f' ·{c + 1}'; l2 = json.dumps(d, ensure_ascii=False) + '\n'
        elif e.get('$') == 'ThreadCreated' or (e.get('$') in ('SessionSet', 'ModesSet', 'WorktreeSkipped', 'WorktreeSet', 'ProviderSet') and e.get('id') in FOLD):
            continue
        out.write(l2); n += 1
print(n, 'events')
