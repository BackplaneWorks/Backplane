#!/usr/bin/env python3
"""analyze.py samples.jsonl [warm_s=1800] [--json out.json]
Memory floor, leaks and latency of a soak. Input: the samples run.mjs writes (30 s apart, several hops chained by T).
RSS floor = the minimum RSS in each 10-minute bin after the warm-up (the sawtooth's bottom). The slope is fitted three ways:
  OLS on the bins, with a 95% CI widened for the lag-1 autocorrelation of the residuals (n_eff = n (1-r)/(1+r));
  Theil-Sen (median of pairwise slopes) with a bootstrap CI over bins;
  and the floor against the log's event count (bytes of floor per committed event), which separates growth that is the log's index
  (28 B/event is designed) from growth over time at a constant log.
Verdict rule: 'rising' if the 95% lower bound of the time slope is above 0 AND the slope is more than 1 MB/h; else 'no rising floor'."""
import json, sys, math, random, statistics as st
args = [a for a in sys.argv[1:] if not a.startswith('--')]
out = sys.argv[sys.argv.index('--json') + 1] if '--json' in sys.argv else None
if out and out in args: args.remove(out)
S = [json.loads(l) for l in open(args[0]) if l.strip()]
warm = float(args[1]) if len(args) > 1 else 1800.0
T = lambda s: s.get('T', s['t'])
S.sort(key=T)
_T = {1:12.706,2:4.303,3:3.182,4:2.776,5:2.571,6:2.447,7:2.365,8:2.306,9:2.262,10:2.228,11:2.201,12:2.179,13:2.160,14:2.145,15:2.131,16:2.120,17:2.110,18:2.101,19:2.093,20:2.086,21:2.080,22:2.074,23:2.069,24:2.064,25:2.060,26:2.056,27:2.052,28:2.048,29:2.045,30:2.042}
def tcrit(df):  # two-sided 95% Student-t quantile for df degrees of freedom (df = n-2 of the fit, or n_eff-2)
    df = max(1, int(round(df)))
    return _T[df] if df <= 30 else (2.021 if df <= 40 else 2.0 if df <= 60 else 1.98 if df <= 120 else 1.96)
def ols(xs, ys):
    n = len(xs); mx = sum(xs)/n; my = sum(ys)/n; sxx = sum((x-mx)**2 for x in xs)
    if sxx == 0 or n < 3: return None
    b = sum((x-mx)*(y-my) for x, y in zip(xs, ys))/sxx; a = my - b*mx
    res = [y-a-b*x for x, y in zip(xs, ys)]
    s2 = sum(r*r for r in res)/max(1, n-2)
    r1 = sum(res[i]*res[i+1] for i in range(n-1))/max(1e-12, sum(r*r for r in res)) if n > 3 else 0
    r1 = max(min(r1, 0.95), 0)
    neff = max(3, n*(1-r1)/(1+r1))  # the CI uses df = n_eff-2 (>= 1)
    se_naive = math.sqrt(s2/sxx); se = se_naive*math.sqrt(n/neff)
    return dict(slope=b, intercept=a, n=n, ci95=tcrit(int(neff-2))*se, ci95_naive=tcrit(n-2)*se_naive, lag1=r1, n_eff=neff)
def theil_sen(xs, ys, boot=400):
    n = len(xs)
    if n < 4: return None
    sl = lambda idx: st.median([(ys[j]-ys[i])/(xs[j]-xs[i]) for a, i in enumerate(idx) for j in idx[a+1:] if xs[j] != xs[i]])
    # slopes of a subsample of pairs when n is big
    pairs = [(i, j) for i in range(n) for j in range(i+1, n) if xs[j] != xs[i]]
    if len(pairs) > 20000: random.Random(1).shuffle(pairs); pairs = pairs[:20000]
    b = st.median([(ys[j]-ys[i])/(xs[j]-xs[i]) for i, j in pairs])
    # block bootstrap over contiguous blocks of bins (keeps the autocorrelation)
    rnd = random.Random(2); blk = max(2, int(round(n**0.5))); bs = []
    for _ in range(boot):
        idx = []
        while len(idx) < n:
            s0 = rnd.randrange(0, n-blk+1) if n > blk else 0
            idx += list(range(s0, min(n, s0+blk)))
        idx = idx[:n]
        pp = [(i, j) for i in idx for j in idx if i < j and xs[j] != xs[i]]
        rnd.shuffle(pp); pp = pp[:600]
        if pp: bs.append(st.median([(ys[j]-ys[i])/(xs[j]-xs[i]) for i, j in pp]))
    bs.sort()
    return dict(slope=b, lo=bs[int(.025*len(bs))], hi=bs[int(.975*len(bs))-1], n=n)
def floors(key, binS=600, warm=warm):
    bins = {}
    tmax = max(T(s) for s in S)
    first_bin = int(math.ceil(warm/binS))   # start at a bin boundary: no partial first bin
    for s in S:
        k = int(T(s)//binS)
        if k < first_bin or key not in s or s[key] != s[key]: continue
        if (k+1)*binS > tmax + 31: continue       # the last bin must be covered to its end (30 s sampling)
        bins.setdefault(k, []).append(s)
    pts = []
    for k, v in sorted(bins.items()):
        m = min(v, key=lambda s: s[key]); pts.append(((k*binS+binS/2)/3600, m[key]/1024, m.get('events', 0)))
    return pts
res = dict(file=args[0], samples=len(S), duration_h=(T(S[-1])-T(S[0]))/3600 if S else 0, warm_s=warm)
print(f"{len(S)} samples over {res['duration_h']:.2f} h, hops {sorted(set(s.get('hop',1) for s in S))}, warm-up {warm/60:.0f} min")
for key, label in (('rss', 'hub RSS'), ('treeRss', 'hub+children RSS'), ('polRss', 'policy child RSS')):
    pts = floors(key)
    if len(pts) < 4: continue
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]; ev = [p[2]/1e6 for p in pts]
    o = ols(xs, ys); ts = theil_sen(xs, ys); oe = ols(ev, ys) if max(ev) > min(ev) else None
    rising = o and (o['slope']-o['ci95'] > 0) and o['slope'] > 1.0
    d = dict(bins=len(pts), floor_first_MB=ys[0], floor_last_MB=ys[-1], floor_min_MB=min(ys), floor_max_MB=max(ys), ols_MB_per_h=o and o['slope'], ols_ci95=o and o['ci95'], ols_ci95_naive=o and o['ci95_naive'], lag1=o and o['lag1'], n_eff=o and o['n_eff'],
             theil_sen_MB_per_h=ts and ts['slope'], ts_lo=ts and ts['lo'], ts_hi=ts and ts['hi'], MB_per_Mevents=oe and oe['slope'], MB_per_Mevents_ci95=oe and oe['ci95'], verdict='rising' if rising else 'no rising floor')
    res[key+'_floor'] = d
    print(f"\n{label} floor ({len(pts)} ten-minute minima): {ys[0]:.0f} MB first -> {ys[-1]:.0f} MB last (min {min(ys):.0f}, max {max(ys):.0f})")
    if o: print(f"  OLS slope {o['slope']:+.2f} MB/h  95% CI +-{o['ci95']:.2f} (lag-1 autocorr {o['lag1']:.2f}, n_eff {o['n_eff']:.1f}; naive +-{o['ci95_naive']:.2f})")
    if ts: print(f"  Theil-Sen {ts['slope']:+.2f} MB/h  95% block-bootstrap [{ts['lo']:+.2f}, {ts['hi']:+.2f}]")
    if oe: print(f"  against the log: {oe['slope']:+.1f} MB per million events +-{oe['ci95']:.1f}  ({oe['slope']*1024*1024/1e6:+.1f} B/event)")
    print(f"  verdict: {d['verdict']}  (JS hub floor, 2.09 h run: +76 MB/h +-9)")
# first and last quarter comparison of the 1-minute RSS minima
def q(key):
    v = [s[key]/1024 for s in S if T(s) >= warm and key in s]
    if len(v) < 40: return
    n = len(v)//4; a = sorted(v[:n]); b = sorted(v[-n:]);
    print(f"{key}: first quarter median {st.median(a):.0f} MB p5 {a[len(a)//20]:.0f} | last quarter median {st.median(b):.0f} MB p5 {b[len(b)//20]:.0f}")
    res[key+'_quarters'] = dict(first_median=st.median(a), first_p5=a[len(a)//20], last_median=st.median(b), last_p5=b[len(b)//20])
print(); q('rss'); q('treeRss')
# counts that must stay flat
for key in ('fds', 'sockets', 'threads', 'kids', 'zombies', 'standins'):
    d = [(T(s)/3600, s[key]) for s in S if T(s) >= warm and key in s and s[key] == s[key]]
    if len(d) < 6: continue
    o = ols(*zip(*d)); vals = [v for _, v in d]
    res[key] = dict(min=min(vals), max=max(vals), slope_per_h=o and o['slope'], ci95=o and o['ci95'])
    print(f"{key:9s} min {min(vals):g} max {max(vals):g}  slope {o['slope']:+.3f}/h +-{o['ci95']:.3f}" if o else f"{key} min {min(vals)} max {max(vals)}")
# log growth
ev = [(T(s)/3600, s['events']) for s in S if 'events' in s]; lb = [(T(s)/3600, s['logBytes']) for s in S if 'logBytes' in s]
if len(ev) > 4: o = ols(*zip(*ev)); res['events_per_h'] = o['slope']; print(f"\nevents {ev[0][1]} -> {ev[-1][1]}  ({o['slope']:.0f}/h)")
if len(lb) > 4: o = ols(*zip(*lb)); res['log_MB_per_h'] = o['slope']/1e6; print(f"events.jsonl {lb[0][1]/1e6:.1f} -> {lb[-1][1]/1e6:.1f} MB ({o['slope']/1e6:.1f} MB/h); home {S[-1].get('homeBytes',0)/1e6:.1f} MB; disk free {S[-1].get('diskFreeKb',0)/1e6:.1f} GB")
cp = [s['cpuPct'] for s in S if T(s) >= warm and 'cpuPct' in s]
if cp: res['cpu'] = dict(median=st.median(cp), p95=sorted(cp)[int(.95*len(cp))-1], max=max(cp)); print(f"hub CPU (percent of one core, 30 s windows): median {res['cpu']['median']:.1f}  p95 {res['cpu']['p95']:.1f}  max {res['cpu']['max']:.1f}")
# latency: merge the histograms
def merged(key, lo=0, hi=1e18):
    h = {}; n = 0; mx = 0
    for s in S:
        if not (lo <= T(s) < hi) or key not in s: continue
        for b, c in s[key].get('h', {}).items(): h[int(b)] = h.get(int(b), 0) + c; n += c
        if s[key].get('max') == s[key].get('max') and s[key].get('max'): mx = max(mx, s[key]['max'])
    return h, n, mx
def hp(h, n, p):
    if not n: return float('nan')
    acc = 0
    for b in sorted(h):
        acc += h[b]
        if acc >= p/100*n: return math.exp(b*math.log(1.02))
    return float('nan')
print("\nlatency (ms), whole run after warm-up; percentiles from merged 2%-bucket histograms, max exact:")
span = (warm, 1e18); res['latency'] = {}
for key, label in (('ping', 'ping round trip'), ('lag', 'delta lag (stand-in stamp -> client)'), ('ack', 'durable write ack (project.add)'), ('view', 'view.update round trip')):
    h, n, mx = merged(key, *span)
    if not n: continue
    r = dict(n=n, p50=hp(h, n, 50), p95=hp(h, n, 95), p99=hp(h, n, 99), p999=hp(h, n, 99.9), max=mx)
    # drift: first vs last quarter of the run
    t0, t1 = T(S[0]), T(S[-1]); qq = (t1-t0)/4
    a = merged(key, max(warm, t0), t0+qq); b = merged(key, t1-qq, 1e18)
    r['p50_first_quarter'] = hp(a[0], a[1], 50); r['p50_last_quarter'] = hp(b[0], b[1], 50); r['p95_first_quarter'] = hp(a[0], a[1], 95); r['p95_last_quarter'] = hp(b[0], b[1], 95)
    res['latency'][key] = r
    print(f"  {label:38s} n={n:>9}  p50 {r['p50']:8.3f}  p95 {r['p95']:8.3f}  p99 {r['p99']:8.3f}  p99.9 {r['p999']:9.3f}  max {mx:9.1f}   p95 first/last quarter {r['p95_first_quarter']:.3f} / {r['p95_last_quarter']:.3f}")
errs = sum(s['ack']['fail'] for s in S if 'ack' in s); verrs = sum(s['view']['err'] for s in S if 'view' in s)
mins = [s['minPerThreadPerS'] for s in S if T(s) >= warm and 'minPerThreadPerS' in s]
res['ack_failures'] = errs; res['view_errors'] = verrs
print(f"write ack failures {errs}, view errors {verrs}; slowest stream, deltas/s in a 30 s window: min {min(mins) if mins else float('nan'):.2f}, median {st.median(mins) if mins else float('nan'):.2f}")
if out: json.dump(res, open(out, 'w'), indent=1, default=lambda x: None)
# per-hop view (chained soak): each hop restarts the hub, so the floor is also read inside every hop, and at each hop's start against the log size
hops = sorted(set(s.get('hop', 1) for s in S))
if len(hops) > 1:
    print("\nper hop: restart RSS and the floor slope inside the hop (10-minute minima, first 10 minutes of the hop left out)")
    print("  hop   events at start   boot RSS MB   floor first -> last MB   slope MB/h (95%)")
    res['hops'] = []
    for hp_ in hops:
        H = [s for s in S if s.get('hop', 1) == hp_]
        bins = {}
        for s in H:
            if s['t'] < 600: continue
            bins.setdefault(int(s['t']//600), []).append(s['rss']/1024)
        fl = [((k*600+300)/3600, min(v)) for k, v in sorted(bins.items())]
        o = ols(*zip(*fl)) if len(fl) >= 4 else None
        first = H[0]
        print(f"  {hp_:>3}   {first['events']:>15}   {first['rss']/1024:>10.1f}   " + (f"{fl[0][1]:>8.1f} -> {fl[-1][1]:<8.1f}   {o['slope']:+7.2f} ({o['ci95']:.2f})" if o else "-"))
        res['hops'].append(dict(hop=hp_, events_at_start=first['events'], start_rss_MB=first['rss']/1024, slope=o and o['slope'], ci95=o and o['ci95']))
    ev_ = [(h_['events_at_start']/1e6, h_['start_rss_MB']) for h_ in res['hops']]
    if len(ev_) >= 4:
        o = ols(*zip(*ev_)); print(f"  boot RSS vs log size: {o['slope']:+.1f} MB per million events (+-{o['ci95']:.1f}) = {o['slope']*1048576/1e6:+.1f} B/event (design: 18.7)"); res['boot_rss_MB_per_Mevents'] = o['slope']
    if out: json.dump(res, open(out, 'w'), indent=1, default=lambda x: None)

if len(hops) > 2 and res.get('hops'):
    sl = [h_['slope'] for h_ in res['hops'] if h_.get('slope') is not None]
    if len(sl) >= 3:
        m = sum(sl)/len(sl); sd = (sum((x-m)**2 for x in sl)/(len(sl)-1))**.5; ci = tcrit(len(sl)-1)*sd/len(sl)**.5
        res['chain_floor_slope_mean_MB_per_h'] = m; res['chain_floor_slope_ci95'] = ci
        v = 'rising inside the hops' if (m-ci > 0 and m > 1.0) else 'no rising floor inside the hops'
        res['chain_verdict'] = v
        print(f"\nchain verdict (mean of the per-hop floor slopes, restarts not pooled): {m:+.2f} MB/h +-{ci:.2f} over {len(sl)} hops -> {v}")
        if out: json.dump(res, open(out, 'w'), indent=1, default=lambda x: None)
