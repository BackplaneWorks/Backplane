#!/usr/bin/env python3
"""pos.py WIN_LOG ID: the centre of the control ID, from the rectangles the client logs with BACKPLANE_GPUI_RECTS=1 (the newest line naming it)."""
import json, re, sys
log, want = sys.argv[1], sys.argv[2]
found = None
for line in open(log, errors="replace"):
    if want not in line or "GPUI_" not in line: continue
    m = re.search(r"\{.*\}", line)
    if not m: continue
    try: v = json.loads(m.group(0))
    except ValueError: continue
    for r in (v.get("rects") or [v]):
        if r.get("id") == want or r.get("name") == want: found = r
if not found: sys.exit(f"no rectangle for {want}")
print(int(found["x"] + found["w"] / 2), int(found["y"] + found["h"] / 2))
