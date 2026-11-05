#!/usr/bin/env python3
"""md_dump.bend's copied Mob.nodes block must equal src/mobile/screen.bend's text."""
import sys
d = open('test/gpui/screen/md_dump.bend').read(); s = open('src/mobile/screen.bend').read()
blk = d.split("# BEGIN COPY\n")[1].split("# END COPY")[0].strip()
if blk not in s: sys.exit("md_dump.bend's copy of Mob.nodes differs from src/mobile/screen.bend: refresh it (see md_dump.bend)")
print("copy ok")
