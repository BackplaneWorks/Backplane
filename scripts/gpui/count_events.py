#!/usr/bin/env python3
"""Counts only, no content: python3 scripts/gpui/count_events.py EVENTS.jsonl
Prints integers and event kind names / setting-key prefixes (the part before the first '.' or ':'). Never prints a value, id, text or full key."""
import re, collections, sys
pat = re.compile(rb'^\{"\$":"([A-Za-z]+)"')
th = re.compile(rb'"thread":"([^"]*)"')
key = re.compile(rb'"key":"([^"]*)"')
c = collections.Counter(); n = 0; pos = []; with_entries = set(); keys = set(); keylen = 0; pre = collections.Counter(); bad = 0
with open(sys.argv[1], "rb") as f:
    for l in f:
        n += 1
        m = pat.match(l)
        k = m.group(1).decode() if m else "?"
        if not m: bad += 1
        c[k] += 1
        if k == "ThreadCreated": pos.append(n)
        elif k in ("MessagePosted", "ActivityLogged"):
            t = th.search(l)
            if t: with_entries.add(hash(t.group(1)))
        elif k == "SettingSet":
            m2 = key.search(l)
            if m2:
                kk = m2.group(1)
                if kk not in keys:
                    keys.add(kk); keylen += len(kk)
                    pre[re.split(rb"[.:]", kk, maxsplit=1)[0].decode("ascii", "replace")] += 1
print("lines", n, "kinds", len(c), "not-an-event lines", bad)
for k, v in c.most_common(10): print(k, v)
print("ThreadCreated", len(pos))
print("threads created by 25/50/75/100% of events", [sum(1 for p in pos if p <= n * i // 4) for i in (1, 2, 3, 4)])
print("threads with entries", len(with_entries))
e = c["MessagePosted"] + c["ActivityLogged"]; print("entries", e, "share", round(e / n, 3))
print("SettingSet events", c["SettingSet"], "distinct keys", len(keys), "mean key bytes", round(keylen / max(1, len(keys)), 1))
print("distinct keys by prefix (top 8)", pre.most_common(8))
