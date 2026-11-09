#!/usr/bin/env python3
"""iOS half of the layout check: the XCUITest audit (ios/UITests/LayoutTests.swift) writes the accessibility tree of each screen as JSON; this judges it with the
same rules as the Android check (android_check.py: overlap, clip-x, clip-y, touch, hscroll, offscreen, missing), with iOS's 44 pt touch target (points are the unit: dpi 160).
A snapshot is {"screen": [w, h] pt, "name": scenario, "tree": {"type", "label", "id", "frame": [x, y, w, h], "enabled", "children": [..]}}.
`python3 ios_check.py` runs the self-test; `python3 ios_check.py DIR` judges every DIR/**/layout-*.json the audit wrote and prints findings as JSON (exit 1 when any is new)."""
import json, sys, os, glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import android_check as ck

CLICK = {'button', 'cell', 'link', 'switch', 'toggle', 'tab', 'segmentedControl', 'menuItem', 'textField', 'secureTextField', 'searchField', 'textView', 'slider', 'stepper', 'checkBox', 'radioButton', 'key'}
SCROLL = {'scrollView', 'table', 'collectionView', 'webView'}
TEXT = {'staticText', 'button', 'textField', 'secureTextField', 'link', 'cell', 'searchField', 'textView', 'switch', 'tab'}
PKG = 'app'

def to_tree(snap):
    """a snapshot as android_check's tree: one window of the app, nodes in pre-order, bounds in points"""
    W, H = snap['screen']; nodes = []
    def walk(e, parent):
        f = e.get('frame') or [0, 0, 0, 0]; t = e.get('type', '')
        # a container that merges its children's label (a cell, a button holding a staticText) carries the text itself: android_check skips a text node inside its ancestor's box
        label = (e.get('label') or '').strip()
        node = {'cls': t, 'pkg': PKG, 'text': label if t in TEXT and label else '', 'rid': e.get('id', ''), 'b': [round(f[0]), round(f[1]), round(f[0] + f[2]), round(f[1] + f[3])],
                'click': t in CLICK and e.get('enabled', True), 'scroll': t in SCROLL, 'enabled': e.get('enabled', True), 'i': len(nodes), 'parent': parent, 'kids': 0, 'edit': t in ('textField', 'textView', 'searchField')}
        nodes.append(node)
        if parent is not None: nodes[parent]['kids'] += 1
        for c in e.get('children', []): walk(c, node['i'])
    walk(snap['tree'], None)
    return {'win': [W, H], 'dpi': 160, 'pkg': PKG, 'min_dp': 44.0, 'windows': [{'id': '0', 'pkg': PKG, 'b': [0, 0, W, H], 'layer': 0, 'title': '', 'nodes': nodes}]}

def check(snap, scen=None):
    return ck.check(to_tree(snap), dict(scen or {}, name=snap.get('name', '')))

if __name__ == '__main__':
    if len(sys.argv) > 1:
        known = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ios-known.json'))) if os.path.exists(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ios-known.json')) else {}
        out = []
        for f in sorted(glob.glob(os.path.join(sys.argv[1], '**', 'layout-*.json'), recursive=True)):
            s = json.load(open(f)); out += [dict(x, file=os.path.relpath(f, sys.argv[1])) for x in check(s, s.get('scen'))]
        new, old = ck.split_known(out, known); json.dump({'new': new, 'known': old}, sys.stdout, indent=1); sys.exit(1 if new else 0)
    E = lambda t, f, label='', kids=(), **k: dict({'type': t, 'label': label, 'frame': f, 'children': list(kids)}, **k)
    S = lambda *kids, scr=(390, 844): {'screen': list(scr), 'name': 't', 'tree': E('window', [0, 0, *scr], kids=[E('other', [0, 0, *scr], kids=kids)])}
    R = lambda s: sorted({f['rule'] for f in check(s)})
    assert R(S(E('staticText', [16, 60, 200, 20], 'Threads'), E('button', [300, 50, 60, 44], 'Add'))) == []
    assert R(S(E('staticText', [16, 60, 200, 20], 'MODEL'), E('staticText', [100, 62, 120, 20], 'EFFORT'))) == ['overlap']
    assert R(S(E('button', [300, 50, 30, 30], 'Add'))) == ['touch']
    assert R(S(E('staticText', [350, 60, 120, 20], 'runs off the right edge'))) == ['clip-x']
    assert R(S(E('staticText', [16, 830, 100, 30], 'under the home bar'))) == ['clip-y']
    assert R(S(E('scrollView', [0, 0, 390, 844], kids=[E('staticText', [16, 820, 100, 30], 'a row below the fold')]))) == []
    assert R(S(E('button', [10, 10, 200, 44], 'Row title', kids=[E('staticText', [16, 20, 100, 20], 'Row title')]))) == []
    assert R(S(E('staticText', [16, 60, 100, 20], 'x'), scr=(390, 844))) == []
    print('ios_check.py: self-test ok')
