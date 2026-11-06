#!/usr/bin/env python3
"""Fit RSS slope (floor) after warm-up. usage: fit.py samples.jsonl [warm_s=900]
Reports least-squares slope of raw RSS and of the per-10-minute MINIMUM (the floor), MB/h with 95% CI."""
import json,sys,math
S=[json.loads(l) for l in open(sys.argv[1])];warm=float(sys.argv[2]) if len(sys.argv)>2 else 900
def fit(xs,ys):
    n=len(xs);mx=sum(xs)/n;my=sum(ys)/n;sxx=sum((x-mx)**2 for x in xs)
    b=sum((x-mx)*(y-my) for x,y in zip(xs,ys))/sxx;a=my-b*mx
    res=sum((y-a-b*x)**2 for x,y in zip(xs,ys));se=math.sqrt(res/max(1,n-2)/sxx);return b,se,n
for key in('rss','treeRss','fds','sockets','kids','thr','events.jsonl','store.db'):
    d=[(s['t']/3600,s[key]/(1024 if key in('rss','treeRss') else 1)) for s in S if s['t']>=warm and key in s]
    if len(d)<6: continue
    b,se,n=fit(*zip(*d));print(f"{key:14s} raw slope {b:10.2f} /h  +-{1.96*se:8.2f} (95%, naive, n={n})")
d=[(s['t'],s['rss']/1024) for s in S if s['t']>=warm]
if d:
    bins={}
    for t,v in d: bins.setdefault(int(t//600),[]).append(v)
    fl=[((k*600+300)/3600,min(v)) for k,v in sorted(bins.items())]
    if len(fl)>=4:
        b,se,n=fit(*zip(*fl));print(f"rss floor (10-min minima) slope {b:8.2f} MB/h +-{1.96*se:.2f} (95%, n={n} bins)")
        print("floors MB:",[round(v) for _,v in fl])
