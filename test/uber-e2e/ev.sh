#!/bin/sh
# ev.sh RUN: the window's metrics by name: clicks and errors, no text
grep -a 'GPUI_METRIC' "$1/window.log" | python3 -c "
import sys,json,collections
c=collections.Counter()
for l in sys.stdin:
    try: e=json.loads(l.split('GPUI_METRIC ',1)[1])
    except: continue
    c[e.get('event')]+=1
    if e.get('event') in('click','not_usable','script_error','error'): print({k:v for k,v in e.items() if k in('event','found','changed','action','ok','error','why')})
print(c.most_common(16))"
