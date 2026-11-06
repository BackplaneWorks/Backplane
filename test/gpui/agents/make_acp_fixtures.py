#!/usr/bin/env python3
"""Turn record_acp.mjs runs into fixtures for the Rust provider tests: the JS hub's events for the agent thread (policy settings dropped)
and what the stand-in received (its start line and the JSON-RPC lines the hub wrote), host-specific text left for the test to normalise.
usage: make_acp_fixtures.py RUNDIR OUTDIR"""
import json, glob, os, sys
run, out = sys.argv[1:3]
TH = 't1700000009-9'
def norm_in(l):
    '''IN lines: the host's folder, the MCP address and the runtime prompt (host-specific) replaced by markers; the Rust test does the same.'''
    if not l.startswith('IN '): return l
    o = json.loads(l[3:])
    p = o.get('params') or {}
    if 'cwd' in p: p['cwd'] = 'CWD'
    for m in p.get('mcpServers') or []: m['url'] = 'MCP'
    for c in p.get('prompt') or []:
        t = c.get('text', '')
        if t.startswith('<backplane_instructions>'): c['text'] = '<I>' + t.split('</backplane_instructions>', 1)[1]
    return 'IN ' + json.dumps(o, separators=(',', ':'), ensure_ascii=False)

POLICY = ['liveness.', 'autosettle.', 'alert.', 'told.', 'summary.at.', 'star.nudge', 'machine.id', 'accounts.', 'allowance.', 'tz.']
for f in sorted(glob.glob(f'{run}/js-*.jsonl')):
    scen = os.path.basename(f)[3:-6]
    ev = []
    for l in open(f):
        if not l.strip(): continue
        e = json.loads(l); k = e.get('key', '')
        if any(k.startswith(p) for p in POLICY): continue
        if TH in l or k.startswith('stopped.') or k.startswith('failed.'): ev.append(e)
    raw = open(f'{run}/fake-{scen}.log').read().split('\n')
    # the hub probes `grok models`, `opencode models`, `--version`... at start: the turn begins at the first START that is the agent itself
    first = next(i for i, l in enumerate(raw) if l.startswith('START ') and (l.endswith(' stdio') or l.endswith('ARGS acp')))
    fake = [l for l in raw[first:] if l.startswith(('START ', 'ENV ', 'IN '))]
    fake = [norm_in(l) for l in fake]
    json.dump({'events': ev, 'fake': fake}, open(f'{out}/acp-{scen}.json', 'w'), separators=(',', ':'), ensure_ascii=False)
    print(scen, len(ev), 'events', len(fake), 'stand-in lines')
