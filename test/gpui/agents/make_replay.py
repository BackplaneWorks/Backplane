#!/usr/bin/env python3
"""Turn a record_modes.mjs run into fixtures: the lines the stand-in wrote with each command uuid replaced by __U<n>__ (n: the order
the stand-in received them), and the JS hub's events for the thread. usage: make_replay.py RUNDIR NAME OUTDIR"""
import json, re, sys
run, name, out = sys.argv[1:4]
fake = open(f'{run}/work-{name}/fake.log').read().split('\n')
uuids = []
for l in fake:
    if l.startswith('IN '):
        try: o = json.loads(l[3:])
        except Exception: continue
        if o.get('type') == 'user' and o.get('uuid') and o['uuid'] not in uuids: uuids.append(o['uuid'])
lines = [l for l in open(f'{run}/lines-{name}.jsonl').read().split('\n') if l.startswith('{')]  # (the --version probe's text is not a stream line)
txt = '\n'.join(lines)
for i, u in enumerate(uuids, 1): txt = txt.replace(u, f'__U{i}__')
open(f'{out}/replay-{name}.ndjson', 'w').write(txt if txt.endswith('\n') else txt + '\n')
ev = [json.loads(l) for l in open(f'{run}/js-{name}.jsonl') if l.strip()]
keep = []
for e in ev:
    k = e.get('key', '')
    if any(k.startswith(p) for p in ['autosettle', 'liveness', 'alert.', 'told.', 'summary.at', 'star.nudge', 'machine.id', 'accounts.', 'allowance.codex', 'tz.']): continue
    if 't1700000001-2' in json.dumps(e) or k.startswith('stopped.') or k.startswith('failed.'): keep.append(e)
open(f'{out}/js-{name}.jsonl', 'w').write('\n'.join(json.dumps(e, separators=(',', ':'), ensure_ascii=False) for e in keep) + '\n')
print(name, len(uuids), 'commands', len(keep), 'events')
