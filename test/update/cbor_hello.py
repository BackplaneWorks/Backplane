#!/usr/bin/env python3
"""/hello as a remote client reads it: CBOR on stdin (a map of text to text or small ints); prints `<version> <commit>`."""
import sys
b=sys.stdin.buffer.read(); i=0
def n(ai):
    global i
    if ai<24: return ai
    k={24:1,25:2,26:4,27:8}[ai]; v=int.from_bytes(b[i:i+k],"big"); i+=k; return v
def item():
    global i
    h=b[i]; i+=1; mt, ai = h>>5, h&31
    if mt==0: return n(ai)
    if mt==1: return -1-n(ai)
    if mt==3: l=n(ai); s=b[i:i+l].decode(); i+=l; return s
    if mt==4: return [item() for _ in range(n(ai))]
    if mt==5: return {item(): item() for _ in range(n(ai))}
    if mt==7: return {20:False,21:True,22:None}.get(ai)
    raise ValueError(mt)
try: d=item(); print(d.get("backplane",""), d.get("commit",""))
except Exception: print("")
