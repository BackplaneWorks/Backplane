#!/usr/bin/env python3
"""md_dump's copied Mob.nodes block must equal src/mobile/screen's text."""
import sys
d = open.read(); s = open.read()
blk = d.split("# BEGIN COPY\n")[1].split("# END COPY")[0].strip()
if blk not in s: sys.exit
print("copy ok")
