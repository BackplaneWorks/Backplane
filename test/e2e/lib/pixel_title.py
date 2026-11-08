#!/usr/bin/env python3
"""pixel_title.py SHOT.png REF.png [--save]: is the thread screen's title (top left, "Thread 1") on SHOT the one on REF? Exit 0 if so. With --save, SHOT's title is written to REF (once, on a known-good screen)."""
import sys
from PIL import Image, ImageChops, ImageStat
BOX = (120, 95, 420, 160)
a = Image.open(sys.argv[1]).convert('L').crop(BOX)
if '--save' in sys.argv: a.save(sys.argv[2]); sys.exit(0)
b = Image.open(sys.argv[2]).convert('L')
sys.exit(0 if ImageStat.Stat(ImageChops.difference(a, b)).mean[0] < 4 else 1)
