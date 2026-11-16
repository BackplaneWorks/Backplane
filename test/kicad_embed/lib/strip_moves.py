#!/usr/bin/env python3
"""strip_moves.py CLIENT_LOG: where the strip's buttons (Full size, Full screen, Pop out) were drawn from the client script's "up" mark to its
"before" mark (BACKPLANE_GPUI_RECTS probes): prints the number of distinct places, the number of snapshots, and the first places."""
import json, sys
seen, up, n = set(), False, 0
for l in open(sys.argv[1], errors="ignore"):
    if '"name":"up","event":"mark"' in l: up = True
    if '"name":"before","event":"mark"' in l: break
    if not up or '"event":"probes"' not in l: continue
    j = json.loads(l[l.index("{"):]); r = {x["id"]: x["x"] for x in j["rects"]}
    seen.add(tuple(r.get(k) for k in ("kicad:full", "kicad:screen", "kicad:popout"))); n += 1
print(len(seen), n, sorted(map(str, seen))[:4])
