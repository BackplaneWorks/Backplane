#!/usr/bin/env python3
# ctl.py ID_SUBSTRING -> "x y" centre of the newest controls event's control; ctl.py -l lists ids
import json,sys
last=None
for l in open(__import__('os').environ['WIN_LOG']):
    if l.startswith('GPUI_METRIC') and '"event":"controls"' in l: last=json.loads(l[12:])
cs=last['controls']
if sys.argv[1]=='-l': print('\n'.join(c['id'] for c in cs if c['visible'])); sys.exit()
m=[c for c in cs if sys.argv[1] in c['id'] and c['visible']]
if not m: sys.exit(1)
r=m[0]['rect']; print(int(r['x']+r['w']/2),int(r['y']+r['h']/2))
