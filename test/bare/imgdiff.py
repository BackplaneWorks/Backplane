#!/usr/bin/env python3
"""imgdiff.py A.png B.png [MIN] [WxH+X+Y]: the pixels that differ between two screenshots, within the crop when given (ImageMagick's compare,
no Python imaging needed). Exit 0 when at least MIN (default 40) changed."""
import subprocess, sys
a, b = sys.argv[1], sys.argv[2]; lo = int(sys.argv[3]) if len(sys.argv) > 3 else 40
crop = sys.argv[4] if len(sys.argv) > 4 else ""
if crop:
    a, b = f"{a}[{crop}]", f"{b}[{crop}]"
p = subprocess.run(["compare", "-metric", "AE", "-fuzz", "2%", a, b, "null:"], capture_output=True, text=True)
try:
    n = int(float(p.stderr.strip().split()[0]))
except Exception:
    print(f"compare failed: {p.stderr.strip()[:200]}"); sys.exit(2)
print(f"{n} pixels changed"); sys.exit(0 if n >= lo else 1)
