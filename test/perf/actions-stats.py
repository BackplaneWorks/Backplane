import sys,json,re
unc=[];cac=[];seen=set()
for l in open('/tmp/bp-startup/client.log'):
    if 'GPUI_METRIC' not in l or '"event":"input"' not in l: continue
    d=json.loads(l.split(' ',1)[1]); st=d['step']
    if not st.startswith('select:') or st.endswith(':hub'): continue
    (cac if st in seen else unc).append(d['latency_ms']) if d.get('ok') else None
    seen.add(st)
def p(v,q): v=sorted(v); return round(v[min(len(v)-1,int(q*len(v)))],1) if v else None
print('uncached n',len(unc),'p50',p(unc,.5),'p95',p(unc,.95),'max',round(max(unc),1) if unc else None)
print('cached   n',len(cac),'p50',p(cac,.5),'p95',p(cac,.95),'max',round(max(cac),1) if cac else None)
