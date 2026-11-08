#!/usr/bin/env python3
"""Layout rules over one audited frame (the client's `audit` metric: BACKPLANE_GPUI_AUDIT=1, script step {"audit": name}). Pure; `python3 check.py` runs the self-test.
A frame is {"win":[w,h],"recs":[{"k":"t","s":text,"b":[x,y,w,h],"el":[..],"mask":[..]} | {"k":"d","id":name,"b":[..],"mask":[..]}]} in logical px.
Rules (a finding is {rule, scenario, text, detail}):
  overlap    two painted text runs share area (a run an opaque box covers was dropped by the client already)
  clip-x     a text run reaches past the left/right edge of the area that clips it (cut off with no ellipsis, or off the window)
  clip-y     a text run reaches past the window's bottom/top while its clipping area is the window itself (not a scroll box)
  offscreen  a named box the scenario says must be on screen is not, or is missing
  missing    a named box or text the scenario expects is not there (the model list is gone)
  min-width  a named box is narrower than the scenario's minimum
  hscroll    a named box wider than the window (horizontal overflow)
"""
import json, sys

TOL = 1.5
def inter(a, b):
    w = min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0]); h = min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1])
    return w * h if w > 0 and h > 0 else 0.0
def contains(o, r): return o[0] - TOL <= r[0] and o[1] - TOL <= r[1] and o[0] + o[2] + TOL >= r[0] + r[2] and o[1] + o[3] + TOL >= r[1] + r[3]
def vis(rec):
    """the part of a run that shows: its extent clipped by its mask"""
    b, m = rec['b'], rec['mask']
    x0, y0 = max(b[0], m[0]), max(b[1], m[1]); x1, y1 = min(b[0] + b[2], m[0] + m[2]), min(b[1] + b[3], m[1] + m[3])
    return [x0, y0, max(0, x1 - x0), max(0, y1 - y0)]

def check(frame, scen):
    out = []; W, H = frame['win']; recs = frame['recs']
    texts = [r for r in recs if r['k'] == 't']; divs = {}
    for r in recs:
        if r['k'] == 'd': divs.setdefault(r['id'], []).append(r)
    add = lambda rule, text, detail: out.append({'rule': rule, 'scenario': scen.get('name', ''), 'text': text, 'detail': detail})
    # overlap
    seen = set()
    for i, a in enumerate(texts):
        va = vis(a)
        if va[2] < 1 or va[3] < 1: continue
        for b in texts[i + 1:]:
            vb = vis(b)
            if vb[2] < 1 or vb[3] < 1: continue
            if a['s'] == b['s'] and a['b'] == b['b']: continue
            # a box inside the other that is the same text (a shadow, a highlight copy) is not an overlap
            ar = inter(va, vb)
            ix = [max(va[0], vb[0]), max(va[1], vb[1]), min(va[0] + va[2], vb[0] + vb[2]) - max(va[0], vb[0]), min(va[1] + va[3], vb[1] + vb[3]) - max(va[1], vb[1])]
            if any(contains(o, ix) for o in a.get('occ', []) + b.get('occ', [])): continue   # that part is under an opaque box painted after it
            if ar > 3.0 and ar / max(1.0, min(va[2] * va[3], vb[2] * vb[3])) > 0.15:
                k = tuple(sorted((a['s'], b['s'])))
                if k not in seen: seen.add(k); add('overlap', '%s | %s' % k, 'x%.0f,y%.0f w%.0f h%.0f over x%.0f,y%.0f w%.0f h%.0f' % (*va, *vb))
    # clip
    for t in texts:
        b, m = t['b'], t['mask']
        if b[2] < 1 or m[2] < 1 or m[3] < 1: continue   # a run in a box of no size (a folded sidebar) shows nothing
        v = vis(t)
        if v[2] < 1 or v[3] < 1: continue   # wholly outside its clip: scrolled out, not cut
        if b[0] < m[0] - TOL or b[0] + b[2] > m[0] + m[2] + TOL:
            add('clip-x', t['s'], 'text x%.0f..%.0f, clip x%.0f..%.0f' % (b[0], b[0] + b[2], m[0], m[0] + m[2]))
        elif (b[1] + b[3] > H + TOL or b[1] < -TOL) and m[1] + m[3] >= H - TOL and m[1] <= TOL:
            add('clip-y', t['s'], 'text y%.0f..%.0f, window h%.0f' % (b[1], b[1] + b[3], H))
    # named boxes
    for name in scen.get('onscreen', []) + scen.get('present', []):
        d = divs.get(name)
        if not d: add('missing', name, 'no box with this id was painted'); continue
        if name in scen.get('onscreen', []):
            for x in d:
                b = x['b']
                if b[0] < -TOL or b[1] < -TOL or b[0] + b[2] > W + TOL or b[1] + b[3] > H + TOL:
                    add('offscreen', name, 'box x%.0f..%.0f y%.0f..%.0f, window %.0fx%.0f' % (b[0], b[0] + b[2], b[1], b[1] + b[3], W, H))
    for name, mn in scen.get('min_w', {}).items():
        for x in divs.get(name, [])[:1]:
            if x['b'][2] < mn - TOL: add('min-width', name, 'width %.0f, needs %.0f' % (x['b'][2], mn))
    for name in scen.get('no_hscroll', []):
        for x in divs.get(name, [])[:1]:
            if x['b'][0] + x['b'][2] > W + TOL: add('hscroll', name, 'right edge %.0f, window %.0f' % (x['b'][0] + x['b'][2], W))
    for s in scen.get('text', []):
        if not any(s in t['s'] for t in texts): add('missing', s, 'text not painted')
    return out

def split_known(findings, known):
    """known: {"rule|text": "why"}; a finding in it is reported but does not fail the run"""
    new, old = [], []
    for f in findings: (old if '%s|%s' % (f['rule'], f['text']) in known else new).append(f)
    return new, old

if __name__ == '__main__':
    T = lambda s, b, mask=None: {'k': 't', 's': s, 'b': b, 'el': b, 'mask': mask or [0, 0, 1000, 700]}
    ok = {'win': [1000, 700], 'recs': [T('AGENT', [10, 10, 50, 14]), T('MODEL', [300, 10, 50, 14]), {'k': 'd', 'id': 'picker', 'b': [5, 5, 900, 300], 'mask': [0, 0, 1000, 700]}]}
    assert not check(ok, {'onscreen': ['picker']})
    bad = {'win': [1000, 700], 'recs': [T('MODEL', [300, 10, 50, 14]), T('EFFORT', [320, 12, 60, 14]), T('wide note that runs on', [900, 100, 200, 14]),
           {'k': 'd', 'id': 'picker', 'b': [700, 5, 400, 300], 'mask': [0, 0, 1000, 700]}]}
    r = {f['rule'] for f in check(bad, {'onscreen': ['picker'], 'present': ['pick-m-0']})}
    assert r == {'overlap', 'clip-x', 'offscreen', 'missing'}, r
    clipped = {'win': [1000, 700], 'recs': [T('row', [10, 100, 400, 14], [0, 0, 200, 700])]}
    assert [f['rule'] for f in check(clipped, {})] == ['clip-x']
    scroll = {'win': [1000, 700], 'recs': [T('row', [10, 690, 100, 14], [0, 0, 1000, 600])]}
    assert not check(scroll, {}), 'a scroll box may cut a row at its bottom'
    hid = {'win': [1000, 700], 'recs': [dict(T('under', [100, 100, 300, 14]), occ=[[200, 90, 400, 100]]), T('over', [250, 100, 60, 14])]}
    assert not check(hid, {}), 'overlap hidden by a box painted over the first run'
    print('check.py: self-test ok')
