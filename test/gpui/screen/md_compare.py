#!/usr/bin/env python3
"""md_compare.py CORPUS BEND RUST: line-by-line JSON equality; prints the first differences with the input."""
import json, sys
corpus = json.load(open(sys.argv[1])); b = open(sys.argv[2]).read().split('\n'); r = open(sys.argv[3]).read().split('\n')
bad = 0; shown = 0
for i, s in enumerate(corpus):
    try: bj = json.loads(b[i]) if i < len(b) else None
    except Exception as e: bj = "unparsable bend line: " + b[i][:80]
    rj = json.loads(r[i]) if i < len(r) and r[i] else None
    if bj != rj:
        bad += 1
        if shown < 25:
            shown += 1; print("--- #%d %r\n bend %s\n rust %s" % (i, s, json.dumps(bj, ensure_ascii=False)[:600], json.dumps(rj, ensure_ascii=False)[:600]))
print("MISMATCHES %d of %d" % (bad, len(corpus)))
