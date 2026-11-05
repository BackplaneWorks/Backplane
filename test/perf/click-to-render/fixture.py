#!/usr/bin/env python3
# fixture.py EVENTS.jsonl > fixture.json : choose the threads the interactions use from a hub's event log
# (a COPY: never point anything at a live home). P = the project with the most threads; A / B = its smallest /
# largest thread (by event count); C = the largest thread of another project, D = the smallest there; FAR = the
# largest thread of a third project (else the largest elsewhere); bot = the newest bot's name.
import collections, json, sys
n = collections.Counter(); proj = {}; title = {}; gone = set(); bots = []; ptitle = {}
for line in open(sys.argv[1], 'rb'):
    try: e = json.loads(line)
    except ValueError: continue
    k = e.get('$')
    if k == 'ThreadCreated': proj[e['id']] = e.get('project', ''); title[e['id']] = e.get('title', '')
    elif k == 'ThreadRenamed': title[e['id']] = e.get('title', '')
    elif k in ('ThreadDeleted', 'ThreadArchived'): gone.add(e['id'])
    elif k == 'ProjectCreated': ptitle[e['id']] = e.get('title', '')
    elif k == 'BotSet' and e.get('name'): bots.append(e['name'])
    t = e.get('thread') or (e.get('id') if k and k.startswith('Thread') else None)
    if isinstance(t, str): n[t] += 1
live = [t for t in proj if t not in gone and proj[t] in ptitle]
full = [t for t in live if n[t] >= 10]  # threads with a conversation (an empty 'New thread' shows nothing)
live = full if len(full) >= 5 else live
by = collections.defaultdict(list)
for t in live: by[proj[t]].append(t)
ps = sorted(by, key=lambda p: -len(by[p]))
size = lambda t: n[t]
P = ps[0]; inP = sorted(by[P], key=size)
other = [p for p in ps[1:] if len(by[p]) >= 2] or ps[1:]
Cp = max(other, key=lambda p: max(map(size, by[p])))
inC = sorted(by[Cp], key=size)
rest = [t for p in ps if p not in (P, Cp) for t in by[p]] or [t for t in live if t not in (inP[0], inP[-1], inC[0], inC[-1])]
FAR = max(rest, key=size)
fx = {'A': inP[0], 'B': inP[-1], 'C': inC[-1], 'D': inC[0], 'FAR': FAR, 'P': P, 'bot': bots[-1] if bots else ''}
fx['titles'] = {fx[k]: title[fx[k]] for k in ('A', 'B', 'C', 'D', 'FAR')}
fx['events'] = {k: size(fx[k]) for k in ('A', 'B', 'C', 'D', 'FAR')}
print(json.dumps(fx, ensure_ascii=False, indent=1))
