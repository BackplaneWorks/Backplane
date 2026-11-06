#!/usr/bin/env python3
"""plot_compare.py ORACLE.json ORACLEDIR RUST.json RUSTDIR: chunk-for-chunk comparison of two plot_corpus.py runs."""
import json, os, sys
oj, od, rj, rd = sys.argv[1:5]
o = {x["rel"]: x for x in json.load(open(oj))}; r = {x["rel"]: x for x in json.load(open(rj))}
def chunks(p):
    t = open(p, "rb").read().decode("utf-8"); i = 0; head = None; cs = []
    while i < len(t):
        j = t.index("\n", i) if t[i] == "b" else None
        if t[i] == "b": head = t[i:j]; i = j + 1; continue
        assert t[i] == "c", t[i:i+20]
        k = i; ns = []
        while len(ns) < 8:
            e = t.index(" ", k); ns.append(t[k + (1 if not ns else 0):e]); k = e + 1
        n = int(ns[6]) + int(ns[7]); cs.append((tuple(ns), t[k:k + n])); i = k + n + 1
    return head, cs
same = diff = cdiff = ctotal = 0; bad = []
for rel, a in o.items():
    b = r.get(rel)
    if b is None: continue
    if a["code"] == b["code"] and a["out"] == b["out"] and a["infos"] == b["infos"]:
        same += 1; continue
    diff += 1
    line = f'{rel}: code {a["code"]}/{b["code"]} out {"same" if a["out"]==b["out"] else "DIFF"} infos {"same" if a["infos"]==b["infos"] else "DIFF"}'
    try:
        ha, ca = chunks(f'{od}/{a["tag"]}.out'); hb, cb = chunks(f'{rd}/{b["tag"]}.out')
        d = sum(1 for x, y in zip(ca, cb) if x != y) + abs(len(ca) - len(cb)); cdiff += d; ctotal += len(ca)
        line += f' head {"same" if ha==hb else ha+" | "+hb} chunks {len(ca)}/{len(cb)} differ {d}'
        fd = next((i for i, (x, y) in enumerate(zip(ca, cb)) if x != y), None)
        if fd is not None: line += f' first {fd}: {ca[fd][0]} | {cb[fd][0]}'
    except Exception as e: line += f" (unparsed: {e})"
    bad.append(line)
print("files compared", same + diff, "identical", same, "different", diff, "chunks differing", cdiff, "of", ctotal)
for l in bad[:40]: print(l)
