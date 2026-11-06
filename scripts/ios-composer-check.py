#!/usr/bin/env python3
"""Asserts on the composer driver's logs (a log nobody compares against passes anything). usage: ios-composer-check.py DIR"""
import re, sys
d = sys.argv[1]
def states(f):
    out = {}
    for l in open(f):
        m = re.search(r'composer \[(\w+)\] view=(.*?) marked=(\w+) sel=(\d+),(\d+) model=(.*?) undo=(\w+)$', l.rstrip())
        if m: out[m[1]] = dict(view=m[2], marked=m[3], model=m[6])
    return out
s = states(d + '/composer-1.log'); r = states(d + '/composer-2.log')
fails = []
def need(c, msg):
    print(('ok   ' if c else 'FAIL ') + msg)
    if not c: fails.append(msg)
a = s.get('a2'); need(a and a['view'] == 'EXTERNAL' and a['model'] == 'EXTERNAL' and a['marked'] == 'false', f'external change during composition applied (a2={a})')
a = s.get('a4'); need(a and a['view'] == '' and a['model'] == '' and a['marked'] == 'false', f'Send during a composition leaves the draft empty (a4={a})')
a = s.get('b1'); need(a and a['view'] == a['model'] and 'NIHAO' in a['view'], f'commit of composed text (b1={a})')
a = s.get('b2'); need(a and a['view'] == a['model'] and 'NIHAO' not in a['view'], f'undo reaches the model (b2={a})')
a = s.get('b3'); need(a and a['view'] == a['model'] and 'pasted' in a['view'], f'text insert (b3={a})')
a = s.get('b4'); need(a and a['view'] == a['model'] and a['view'].startswith('XY'), f'selection replaced (b4={a})')
a = s.get('b6'); need(a and a['view'] == '' and a['model'] == '', f'the send handler clears (hardware Return not driven: @hwreturn calls onSend) (b6={a})')
a = s.get('b7'); need(a and a['view'] == a['model'], f'undo after send keeps view == model (b7={a})')
a = s.get('c1'); need(a and a['view'] == a['model'] == 'draft kept across relaunch', f'draft set (c1={a})')
a = r.get('r1'); need(a and a['view'] == a['model'] == 'draft kept across relaunch', f'draft restored after relaunch (r1={a})')
msgs = [l.rstrip('\n') for l in open(d + '/user-messages.txt')]
base = int(re.search(r'\d+', open(d + '/baseline.txt').read())[0])
sent = [m for m in msgs]
print('user messages at the hub:', len(msgs), 'baseline', base)
need(len(msgs) - base == 2, f'exactly two sends reached the hub (Send mid-composition, Return) (got {len(msgs) - base})')
need(int(open(d + '/attach-count.txt').read().strip() or 0) >= 1, 'pasted image reached the hub')
sys.exit(1 if fails else 0)
