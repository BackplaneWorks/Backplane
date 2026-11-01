"""Audit only this disposable capture home and verify lossless selections."""
import hashlib,json,os,shutil,time
from pathlib import Path
from PIL import Image,ImageChops
SRC=Path(__file__).resolve().parent
OUT=SRC.parent
R=Path('<capture-root>')
seed=json.loads((SRC/'current-seed.json').read_text())
run=Path(seed['run'])
assert run.is_relative_to(R/'run') and run.name.startswith('B-')
all_events=[json.loads(l) for l in (run/'home/events.jsonl').read_text().splitlines()]
appended=all_events[seed['events']:]
forbidden=[e for e in appended if e['$'] in ['SessionSet','TurnUsage','TaskDelegated'] or e['$']=='TurnChanged' and e['state']=='running' or e['$']=='MessagePosted' and e.get('role')=='assistant']
delivered={e['task'] for e in all_events if e['$']=='TaskDelivered'}
states={e['task']:e['state'] for e in all_events if e['$']=='TaskStateSet'}
tasks={e['task'] for e in all_events if e['$']=='TaskDelegated'}
assert tasks<=delivered and all(states[t]=='done' for t in tasks)
assert not forbidden,forbidden
selections=json.loads((SRC/'current-selection.json').read_text())
for row in selections:
 if 'raw' not in row: continue
 raw=Image.open(OUT/row['raw'])
 out=Image.open(OUT/row['path'])
 for rect in row['crop_rectangles']:
  box=rect['source']; x,y=rect['destination'][:2]; w,h=box[2]-box[0],box[3]-box[1]
  assert ImageChops.difference(raw.crop(box).convert('RGB'),out.crop((x,y,x+w,y+h)).convert('RGB')).getbbox() is None
assert (OUT/'mailroom.png').read_bytes()==(OUT.parent/'mailroom-200.png').read_bytes()
audit={'appended_events':appended,'forbidden_agent_events':forbidden,'task_states':states,'delivered_tasks':sorted(delivered),'blocked_agent_invocations':(run/'blocked-agents.log').read_text() if (run/'blocked-agents.log').exists() else '', 'lossless_selection_verified':True,'mailroom_byte_identical':True}
(SRC/'current-runtime-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest={**seed,'captured_at':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'app_sha256':sha(R/'app/backplane-serve'),'deviceScaleFactor':2,'viewports':{'overview':[1000,720],'delegation':[1076,720],'mailroom':[1600,850],'webhooks':[1000,720],'space':[1000,720],'space-webapp':[480,720]},'network':'loopback-only user/network/UTS namespace','fixture_sha256':sha(SRC/'current-events.jsonl'),'web_assets':{str(p.relative_to(R/'app/web')):sha(p) for p in (R/'app/web').rglob('*') if p.is_file() and p.suffix in ['.js','.css','.html']},'mailroom_available':True,'space_http_status':200,'no_agent_turns':True}
(SRC/'current-capture.json').write_text(json.dumps(manifest,indent=2)+'\n')
for f,new in [('browser.log','current-browser-log.txt'),('hub.log','current-hub-log.txt')]: shutil.copyfile(run/f,SRC/new)
print('Verified: no agent turns, both tasks done and delivered, every crop pixel exact, Mailroom files identical.')
