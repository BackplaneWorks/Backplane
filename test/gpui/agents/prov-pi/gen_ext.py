#!/usr/bin/env python3
"""Extract the two extensions Backplane writes for Pi and Oh My Pi from src/core/pi.bend (Pi.ext) and omp.bend (Omp.ext), byte for byte,
into native/host/src/agents/providers/{pi,omp}_ext.js. usage: gen_ext.py [REPO_ROOT]  (--check: compare, write nothing)"""
import re, sys, os
args = [a for a in sys.argv[1:] if not a.startswith('--')]
root = args[0] if args else os.path.join(os.path.dirname(__file__), '..', '..', '..', '..')
def unesc(s):
    out, i = [], 0
    while i < len(s):
        if s[i] == '\\':
            n = s[i + 1]; out.append({'n': '\n', 't': '\t', 'r': '\r'}.get(n, n)); i += 2
        else: out.append(s[i]); i += 1
    return ''.join(out)
def items(path, name, subs):
    t = open(os.path.join(root, path)).read()
    body = t[t.index(f'def {name}() -> String:'):]
    body = body[body.index('String.join([') + len('String.join(['):]
    body = body[:body.index('], "\\n")')]
    lines = []
    for raw in body.split('\n'):
        raw = raw.strip().rstrip(',')
        if not raw: continue
        parts = re.findall(r'"((?:[^"\\]|\\.)*)"|(\S+\([^)]*\))', raw)
        s = ''
        for lit, call in parts:
            s += subs[call] if call else unesc(lit)
        lines.append(s)
    return '\n'.join(lines)
files = {
  'native/host/src/agents/providers/pi_ext.js': items('src/core/pi.bend', 'Pi.ext', {'Pi.approval.title()': 'backplane:approval'}),
  'native/host/src/agents/providers/omp_ext.js': items('src/core/omp.bend', 'Omp.ext', {}),
}
bad = False
for p, txt in files.items():
    full = os.path.join(root, p)
    if '--check' in sys.argv:
        if not os.path.exists(full) or open(full).read() != txt: print('differs:', p); bad = True
    else: open(full, 'w').write(txt); print('wrote', p, len(txt))
sys.exit(1 if bad else 0)
