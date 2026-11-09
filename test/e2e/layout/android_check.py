#!/usr/bin/env python3
"""Layout rules over one Android view tree (`uiautomator dump --windows`), the phone half of check.py. Pure; `python3 android_check.py` runs the self-test.
Findings have check.py's shape {rule, scenario, text, detail}, so the report and the contact sheet are shared.
  parse(xml, pkg)  -> {"win": [w, h] px, "dpi": density, "windows": [{"id", "pkg", "b", "layer", "nodes": [node]}]}   (px; the rules convert to dp)
  check(tree, scen) -> findings. scen = {"name", "text": [str], "popup": bool, "ignore": [rule]}
A node is {"cls", "pkg", "text" (text or content-desc), "rid", "b": [l, t, r, b], "click", "scroll", "enabled", "i", "parent", "kids"}.
Rules:
  overlap       two text-bearing nodes of one window share visible area (neither contains the other, a merged parent and its children are one thing)
  clip-x        a node reaches past the left/right edge of its parent or of the screen, outside a horizontal scroller (cut off, or off the screen)
  clip-y        a text node reaches past the screen's top/bottom while no scrolling ancestor holds it
  touch         an enabled clickable node of the app is smaller than 48 dp in either direction (its width or height, in dp)
  hscroll       a node wider than the screen
  offscreen     a popup/dialog window is not entirely on the screen
  missing       a text the screen must show is not there
  no-frame      the dump had no app window (the app crashed, or the screen did not open)
uiautomator reports bounds clipped to the visible part, so a node cut by the screen edge shows as a node touching it: `edge` is reported as clip-x for text
that touches a screen edge with no room (< 2 dp) while its parent is wider than the screen... only when the parent says so. Icons and drawing without a view are not seen."""
import re, sys, xml.etree.ElementTree as ET

TOL = 2          # px
MIN_DP = 48.0   # Android's; iOS passes 44 (tree['min_dp'])
SYSTEM = ('com.android.systemui', 'com.google.android.inputmethod', 'com.android.inputmethod', 'com.google.android.apps.nexuslauncher')

def _b(s):
    m = re.match(r'\[(-?\d+),(-?\d+)\]\[(-?\d+),(-?\d+)\]', s or '')
    return [int(x) for x in m.groups()] if m else None

def parse(xml, pkg=None, dpi=160, size=None):
    """the dump as windows of flat node lists; with `--windows` each <window> is one; without, the <hierarchy> is one window"""
    root = ET.fromstring(xml[xml.index('<'):])
    wins = []
    def walk(el, win, parent):
        for n in el.findall('node'):
            b = _b(n.get('bounds'))
            if not b: continue
            node = {'cls': n.get('class', ''), 'pkg': n.get('package', ''), 'text': (n.get('text') or n.get('content-desc') or '').strip(), 'rid': n.get('resource-id', ''),
                    'b': b, 'click': n.get('clickable') == 'true' or n.get('checkable') == 'true' or n.get('long-clickable') == 'true', 'scroll': n.get('scrollable') == 'true' or any(k in n.get('class', '') for k in ('ScrollView', 'RecyclerView', 'ListView')),
                    'enabled': n.get('enabled') != 'false', 'i': len(win['nodes']), 'parent': parent, 'kids': 0, 'edit': 'EditText' in n.get('class', '')}
            win['nodes'].append(node)
            if parent is not None: win['nodes'][parent]['kids'] += 1
            walk(n, win, node['i'])
    ws = root.findall('.//window')
    if ws:
        for k, w in enumerate(ws):
            win = {'id': w.get('id', str(k)), 'pkg': '', 'b': _b(w.get('bounds')) or [0, 0, 0, 0], 'layer': int(w.get('layer') or k), 'title': w.get('title', ''), 'nodes': []}
            h = w.find('hierarchy'); walk(w if h is None else h, win, None)
            if win['nodes']: win['pkg'] = win['nodes'][0]['pkg']
            wins.append(win)
    else:
        win = {'id': '0', 'pkg': '', 'b': _b(root.get('bounds')) or [0, 0, 0, 0], 'layer': 0, 'title': '', 'nodes': []}
        walk(root, win, None)
        if win['nodes']: win['pkg'] = win['nodes'][0]['pkg']; win['b'] = win['nodes'][0]['b']
        wins.append(win)
    # the screen
    W, H = size or (wins[0]['b'][2], wins[0]['b'][3])   # `wm size` (rotated) when the driver knows it; else the first window
    return {'win': [W, H], 'dpi': dpi, 'pkg': pkg, 'windows': wins}

def _vis(a, b):
    w = min(a[2], b[2]) - max(a[0], b[0]); h = min(a[3], b[3]) - max(a[1], b[1])
    return (w, h) if w > 0 and h > 0 else (0, 0)

def _anc(win, i):
    out = []; p = win['nodes'][i]['parent']
    while p is not None: out.append(p); p = win['nodes'][p]['parent']
    return out

def modal_drop(win):
    """indexes of the nodes a modal covers: Compose draws a dialog in the activity's own window after the page, so the page stays in the tree under it (with bounds cut to nothing).
    The page is the last top-level sibling that scrolls (or holds a scroller); what follows it is the dialog."""
    nodes = win['nodes']; kids = lambda i: [n['i'] for n in nodes if n['parent'] == i]
    top = [n['i'] for n in nodes if n['parent'] is None]
    while len(top) == 1 and kids(top[0]): top = kids(top[0])
    def scrolls(i): return nodes[i]['scroll'] or any(scrolls(k) for k in kids(i))
    idx = [k for k, i in enumerate(top) if scrolls(i)]
    if not idx or idx[-1] == len(top) - 1: return set()
    drop = set()
    def add(i): drop.add(i); [add(k) for k in kids(i)]
    for i in top[:idx[-1] + 1]: add(i)
    return drop

def check(tree, scen):
    MIN_DP = tree.get('min_dp', 48.0)
    out = []; W, H = tree['win']; dpi = tree['dpi'] or 160; dp = lambda px: px * 160.0 / dpi; pkg = tree.get('pkg')
    add = lambda rule, text, detail: rule not in scen.get('ignore', []) and out.append({'rule': rule, 'scenario': scen.get('name', ''), 'text': text, 'detail': detail})
    app = [w for w in tree['windows'] if (not pkg or w['pkg'] == pkg)]
    if not app:
        add('no-frame', scen.get('name', ''), 'no window of %s in the dump' % pkg); return out
    seen = set(); alltext = []
    for w in app:
        nodes = w['nodes']; gone = modal_drop(w) if scen.get('modal') else set()
        # overlap: leaves with text (a node with text and children is a merged parent: its children are inside it by design)
        big = lambda n: (n['b'][2] - n['b'][0]) * (n['b'][3] - n['b'][1]) > 0.3 * W * H   # a canvas with a description is a surface, not a label
        leaves = [n for n in nodes if n['i'] not in gone and n['text'] and not big(n) and n['b'][2] > n['b'][0] and n['b'][3] > n['b'][1]]
        alltext += [n['text'] for n in leaves]
        for k, a in enumerate(leaves):
            for b in leaves[k + 1:]:
                if a['i'] in _anc(w, b['i']) or b['i'] in _anc(w, a['i']): continue
                vw, vh = _vis(a['b'], b['b'])
                if vw <= TOL or vh <= TOL: continue
                small = min((a['b'][2] - a['b'][0]) * (a['b'][3] - a['b'][1]), (b['b'][2] - b['b'][0]) * (b['b'][3] - b['b'][1]))
                if vw * vh / max(1, small) > 0.15:
                    key = tuple(sorted((a['text'], b['text'])))
                    if key not in seen:
                        seen.add(key); add('overlap', '%s | %s' % key, 'x%.0f..%.0f y%.0f..%.0f dp over x%.0f..%.0f y%.0f..%.0f dp' % tuple(dp(v) for v in (a['b'][0], a['b'][2], a['b'][1], a['b'][3], b['b'][0], b['b'][2], b['b'][1], b['b'][3])))
        for n in nodes:
            b = n['b']; own = n['pkg'] == pkg if pkg else True
            if not own or n['i'] in gone: continue
            par = nodes[n['parent']] if n['parent'] is not None else None
            if n['text']:
                if par is not None and not par['scroll'] and (b[0] < par['b'][0] - TOL or b[2] > par['b'][2] + TOL) and not any(nodes[a]['scroll'] for a in _anc(w, n['i'])):
                    add('clip-x', n['text'], 'node x%.0f..%.0f dp, parent x%.0f..%.0f dp' % (dp(b[0]), dp(b[2]), dp(par['b'][0]), dp(par['b'][2])))
                elif b[0] < -TOL or b[2] > W + TOL:
                    add('clip-x', n['text'], 'node x%.0f..%.0f dp, screen %.0f dp wide' % (dp(b[0]), dp(b[2]), dp(W)))
                elif (b[1] < -TOL or b[3] > H + TOL) and not any(nodes[a]['scroll'] for a in _anc(w, n['i'])):
                    add('clip-y', n['text'], 'node y%.0f..%.0f dp, screen %.0f dp high' % (dp(b[1]), dp(b[3]), dp(H)))
            if b[2] - b[0] > W + TOL and not n['scroll']:
                add('hscroll', n['text'] or n['cls'], 'width %.0f dp, screen %.0f dp' % (dp(b[2] - b[0]), dp(W)))
            if n['click'] and n['enabled'] and not n['scroll'] and (b[2] > b[0] and b[3] > b[1]):
                tw, th = dp(b[2] - b[0]), dp(b[3] - b[1])
                # a clickable whose click area is a bigger clickable's (a row with a button on it) is judged on its own size; the wrapper that holds only it is not
                if min(tw, th) < MIN_DP - 1.0 and not (n['kids'] and not n['text'] and tw >= MIN_DP and th >= MIN_DP):
                    add('touch', n['text'] or n['rid'] or n['cls'].rsplit('.', 1)[-1], '%.0fx%.0f dp at x%.0f y%.0f dp, needs %d' % (tw, th, dp(b[0]), dp(b[1]), MIN_DP))
        # a popup or dialog window (not the activity's own full-screen one) entirely on the screen
        full = w['b'][2] - w['b'][0] >= W - TOL and w['b'][3] - w['b'][1] >= H - TOL
        if not full and (scen.get('popup') or w['layer'] > 0 and len(app) > 1):
            wb = w['b']
            if wb[0] < -TOL or wb[1] < -TOL or wb[2] > W + TOL or wb[3] > H + TOL:
                add('offscreen', w.get('title') or w['id'], 'window x%.0f..%.0f y%.0f..%.0f dp, screen %.0fx%.0f dp' % (dp(wb[0]), dp(wb[2]), dp(wb[1]), dp(wb[3]), dp(W), dp(H)))
    for s in scen.get('text', []):
        if not any(s.lower() in t.lower() for t in alltext): add('missing', s, 'text not in the view tree')
    return out

def split_known(findings, known):
    new, old = [], []
    for f in findings: (old if '%s|%s' % (f['rule'], f['text']) in known else new).append(f)
    return new, old

if __name__ == '__main__':
    def N(text, b, cls='android.widget.TextView', click=False, scroll=False, kids=None, pkg='app'):
        return '<node index="0" text="%s" resource-id="" class="%s" package="%s" content-desc="" clickable="%s" scrollable="%s" enabled="true" bounds="[%d,%d][%d,%d]">%s</node>' % (
            text, cls, pkg, str(click).lower(), str(scroll).lower(), *b, kids or '')
    def D(*nodes, extra=''):
        return '<?xml version="1.0"?><hierarchy rotation="0"><window id="1" bounds="[0,0][1080,2400]" layer="0">%s</window>%s</hierarchy>' % (''.join(nodes), extra)
    R = lambda xml, scen=None: sorted({f['rule'] for f in check(parse(xml, 'app', 420, (1080, 2400)), dict({'name': 't'}, **(scen or {})))})
    root = lambda *k: N('', [0, 0, 1080, 2400], 'android.widget.FrameLayout', kids=''.join(k))
    # clean: two rows apart, a 56 dp (147 px at 420) button
    assert R(D(root(N('Threads', [40, 100, 400, 160]), N('Settings', [600, 100, 1000, 160]), N('Go', [900, 300, 1060, 460], 'android.widget.Button', click=True)))) == []
    # overlap: two labels share area
    assert R(D(root(N('MODEL', [300, 100, 500, 160]), N('EFFORT', [400, 110, 600, 170])))) == ['overlap']
    # a merged parent and its child are one thing, not an overlap
    assert R(D(root(N('Row title', [0, 0, 1080, 200], click=True, kids=N('Row title', [20, 20, 500, 80]))))) == []
    # touch: 24 dp = 63 px button
    assert R(D(root(N('x', [100, 100, 160, 160], 'android.widget.Button', click=True)))) == ['touch']
    # clip-x: a child wider than its parent; and off the screen
    assert R(D(N('', [0, 0, 1080, 2400], 'android.widget.FrameLayout', kids=N('', [100, 0, 600, 200], 'android.view.View', kids=N('a long label that is cut', [300, 10, 900, 70]))))) == ['clip-x']
    # inside a scroller it is not cut, it scrolls
    assert R(D(N('', [0, 0, 1080, 2400], 'android.widget.FrameLayout', kids=N('', [100, 0, 600, 200], 'android.widget.HorizontalScrollView', scroll=True, kids=N('chip', [300, 10, 900, 70]))))) == []
    # clip-y: text past the bottom, no scroller; a list row past the bottom is fine
    assert R(D(root(N('footer', [40, 2380, 400, 2440])))) == ['clip-y']
    assert R(D(root(N('', [0, 0, 1080, 2400], 'androidx.recyclerview.widget.RecyclerView', scroll=True, kids=N('row', [40, 2380, 400, 2440]))))) == []
    # popup partly off the screen
    pop = '<window id="2" bounds="[700,2200][1300,2500]" layer="1">%s</window>' % N('menu item', [720, 2220, 1200, 2300])
    assert R(D(root(N('ok', [10, 10, 100, 70])), extra=pop), {'popup': True}) == ['clip-x', 'offscreen']
    # missing text; and no app window
    assert R(D(root(N('hi', [10, 10, 100, 70]))), {'text': ['Models']}) == ['missing']
    assert R(D(N('', [0, 0, 1080, 2400], pkg='com.android.systemui'))) == ['no-frame']
    # system UI and the keyboard are not the app's
    assert R(D(root(N('hi', [10, 10, 100, 70])), extra='')) == []
    # a modal: the page under a dialog keeps its (cut) bounds in the tree and is not judged
    dlg = root(N('', [0, 0, 1080, 2400], 'android.widget.ScrollView', scroll=True, kids=N('page', [63, 1110, 224, 1131], 'android.widget.Button', click=True)), N('Title', [60, 1000, 400, 1060]), N('Pair', [300, 1300, 440, 1430], 'android.widget.Button', click=True))
    assert R(D(dlg)) == ['touch'] and R(D(dlg), {'modal': True}) == [], (R(D(dlg)), R(D(dlg), {'modal': True}))
    # the real shape: <displays><display><window><hierarchy><node>
    real = '<displays><display id="0"><window id="1" bounds="[0,0][1080,2400]" layer="0"><hierarchy rotation="0">%s</hierarchy></window></display></displays>' % root(N('MODEL', [300, 100, 500, 160]), N('EFFORT', [400, 110, 600, 170]))
    assert R(real) == ['overlap'], R(real)
    print('android_check.py: self-test ok')
