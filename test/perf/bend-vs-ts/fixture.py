"""Anonymized read-model projection of the immutable private snapshot.

Preserves event order/types, identity relationships, entry lengths, roles,
timestamps and metadata cardinalities. No message, path or user setting value
leaves the private build folder. The fixture is not a full replay benchmark.
"""
import collections
import gzip
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
ids = {}

def anon(s):
    if not s:
        return ''
    if s.startswith('@sub:'):
        return '@sub:' + anon(s[5:])
    if s.startswith('@'):
        return '@' + anon(s[1:])
    if s not in ids:
        # IDs keep their length and common-prefix shape, but not their value.
        ids[s] = 'x' + format(len(ids), 'x').zfill(max(1, len(s)-1))
    return ids[s]

def sender(s):
    if s.startswith('t:') and ':' in s[2:]:
        return s[2:].split(':', 1)[0]
    return ''

def msg(s):
    f = sender(s)
    return 't:' + anon(f) + ':' + anon(s.split(':', 2)[2]) if f else anon(s)

prefixes = ['autosettle.' + x + '.' for x in ['quiet','output','read','delivery','report','audience','seed']]
prefixes += ['liveness.', 'launch.', 'launched.']

def key(s):
    if s == 'autosettle.version':
        return s
    for p in prefixes:
        if s.startswith(p):
            return p + anon(s[len(p):])
    return anon(s)

threads, asks, tasks, queued, work = {}, {}, {}, {}, {}
entry_count = 0
event_count = 0
with gzip.open(out/'events.jsonl.gz', 'wt', compresslevel=6) as events, gzip.open(out/'entries.jsonl.gz', 'wt', compresslevel=6) as entries, (root/'private-events.jsonl').open() as source:
    for seq, line in enumerate(source, 1):
        e = json.loads(line)
        tag = e['$']
        event_count += 1
        event = {'$': tag}
        for field in ['thread','id','parent','child','by','source']:
            if field in e:
                event[field] = anon(e[field])
        if 'msg' in e:
            event['msg'] = msg(e['msg'])
        if tag == 'SettingSet':
            event['key'] = key(e['key'])
        # CtxNoted is a legacy record not decoded as a current Change.
        events.write(json.dumps(event, separators=(',',':'))+'\n')
        if tag in ('MessagePosted', 'ActivityLogged', 'Reminded'):
            entry_count += 1
            text = e.get('text', e.get('summary',''))
            identity = e.get('msg', e.get('act', 'remind:'+e.get('id','')))
            # Only length, no text. SQLite materializes synthetic bodies later.
            row = [seq, entry_count, msg(identity), anon(e['thread']), anon(sender(identity)), 'msg' if tag=='MessagePosted' else 'act', e.get('role',e.get('tone','remind')), e.get('kind',''), len(text), e['at']]
            entries.write(json.dumps(row,separators=(',',':'))+'\n')
        if tag == 'ThreadCreated':
            threads[e['id']] = dict(id=anon(e['id']), project=anon(e['project']), env=e['env'], worktree=bool(e['worktree']), turn='idle', pending=0, archived=False, deleted=False, activity=e['at'])
        th = e.get('thread',e.get('id',''))
        if th in threads:
            t = threads[th]
            if tag in ('TurnChanged','MessagePosted','ActivityLogged','ThreadWoke'):
                t['activity']=e['at']
            if tag=='TurnChanged': t['turn']=e['state']
            if tag=='PendingChanged': t['pending']=e['count']
            if tag=='WorktreeSet': t['worktree']=bool(e['worktree'])
            if tag=='ThreadArchived': t['archived']=e.get('archived',False)
            if tag=='ThreadDeleted': t['deleted']=True
        if tag=='AskOpened':
            asks[e['id']]=dict(id=anon(e['id']),thread=anon(e['thread']),kind=e['kind'],open=True)
        if tag=='AskClosed' and e['id'] in asks: asks[e['id']]['open']=False
        if tag=='TaskDelegated':
            tasks[e['task']]=dict(id=anon(e['task']),parent=anon(e['parent']),child=anon(e['child']),state='queued')
        if tag=='TaskStateSet' and e['task'] in tasks:
            t=tasks[e['task']]
            if t['state'] not in ('done','failed','cancelled'): t['state']=e['state']
        if tag=='TurnQueued':
            queued.setdefault(anon(e['thread']),[]).append(('handoff:' if e['msg'].startswith('handoff:') else '')+anon(e['msg']))
        if tag=='QueueTaken': queued[anon(e['thread'])]=[]
        if tag in ('QueueTakenOne','QueueRemoved'):
            qs=queued.get(anon(e['thread']),[])
            target=anon(e.get('msg',''))
            queued[anon(e['thread'])]=[q for q in qs if q!=target]
        if tag=='WorkSet' and e['kind'] in ('bg','watch'):
            # Bg.live counts nonempty lines, including ended rows.
            work[e['kind']+'.'+anon(e['thread'])]='\n'.join('b\twork' for x in e['rows'].split('\n') if x)
        if tag=='SettingSet':
            for p in ('bg.','watch.','order.wait.'):
                if e['key'].startswith(p):
                    value=e['value']
                    work[p+anon(e['key'][len(p):])]=('set' if value else '') if p=='order.wait.' else '\n'.join('b\twork' for x in value.split('\n') if x)
state=dict(threads=sorted(threads.values(),key=lambda t:t['activity'],reverse=True),asks=list(asks.values()),tasks=list(tasks.values()),queued=queued,settings=work)
(out/'metadata.json').write_text(json.dumps(state,separators=(',',':'))+'\n')
summary=dict(events=event_count,entries=entry_count,threads=len(threads),asks=len(asks),tasks=len(tasks),work_keys=len(work),snapshot=json.loads((root/'snapshot.json').read_text())['sha256'])
summary['sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.iterdir()) if p.is_file()}
(out/'manifest.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary))
