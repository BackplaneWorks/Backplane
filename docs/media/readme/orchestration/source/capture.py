"""Run installed Backplane with demo-only events, Xvfb, and a disposable home.
Usage: uv run --with pillow source/capture.py
No agent turns or external API calls. Browser views are read-only navigation.
"""
import hashlib, json, os, socket, subprocess, tempfile, time, shutil
from pathlib import Path
from PIL import Image
BUN = shutil.which('bun')
OUT = Path(__file__).resolve().parent.parent
APP = Path('/home/h/.local/share/backplane/backplane')
XVFB = '/home/h/.local/xvfb-root/usr/bin/Xvfb'
XPOKE = '/home/h/Documents/i2cjak/bp-review/ship4/bin/xpoke'

def main():
    home = Path(tempfile.mkdtemp(prefix='backplane-orchestration-'))
    now = int(time.time())
    events = []
    def add(tag, **kw): events.append({'$':tag, **kw, 'at':now - 120 + len(events)})
    for key in ['onboarded', 'onboard.done', 'onboard.uber']:
        add('SettingSet', key=key, value='1')
    add('SettingSet',key='mail.on',value='on')
    for pid,title in [('sensor','Sensor node'),('gateway','Gateway')]:
        root = home / 'demo-projects' / pid
        root.mkdir(parents=True)
        add('ProjectCreated', id=pid, title=title, root=str(root))
    def thread(tid, project, title, provider='codex', model='gpt-6.1-sol'):
        add('ThreadCreated', id=tid, project=project, title=title, env='local', branch='', worktree='', provider=provider, model=model)
    thread('release','sensor','Coordinate rev C')
    thread('power','sensor','Power budget review','claude','claude-sonnet-5')
    thread('layout','sensor','Layout review','grok','grok-4.7')
    thread('firmware','gateway','BLE handoff','muse','muse-spark-1.3')
    for bid,name,look,persona in [('mochi','Mochi',3,'Coordinates projects, bots and threads.'),('tofu','Tofu',41,'Reviews schematics and power budgets.'),('pixel','Pixel',58,'Keeps track of layout and documentation.')]:
        thread('bt-'+bid,'',name,'claude','claude-sonnet-5')
        add('BotSet',id=bid,name=name,thread='bt-'+bid,look=look,persona=persona)
    for pid,key in [('sensor','4000'),('gateway','8000'),('scratch','c000')]:
        add('ProjectOrdered',id=pid,key=key)
    add('UberSet',bot='mochi',uid='demo-mochi',)
    def msg(tid,role,text):
        add('MessagePosted',thread=tid,msg='m'+str(len(events)),role=role,text=text)
    msg('release','user','Coordinate the sensor rev C review. Delegate the power and layout checks, then collect the results here.')
    msg('release','assistant','I split the review between Claude and Grok. Each task has its own thread; results return here when they finish.')
    for task,child,provider,model in [('power-check','power','claude','claude-sonnet-5'),('layout-check','layout','grok','grok-4.7')]:
        add('TaskDelegated',task=task,parent='release',child=child,depth=1,provider=provider,model=model)
        add('TaskDelivered',task=task)
        add('TaskStateSet',task=task,state='done',summary='Power budget reviewed; ready for integration.' if child=='power' else 'Layout reviewed; connector orientation confirmed.')
    msg('release','assistant','Both reviews are back. Power budget: 18 months at the stated duty cycle. Layout: antenna keepout and connector orientation checked. Tofu has the handoff in its Mailbox.')
    add('TurnChanged',thread='release',state='completed')
    msg('power','assistant','Power budget review complete. Sleep current and radio duty cycle are within the demo targets.')
    add('TurnChanged',thread='power',state='completed')
    msg('layout','assistant','Layout review complete. Antenna keepout and connector orientation checked.')
    add('TurnChanged',thread='layout',state='completed')
    msg('firmware','assistant','BLE handoff prepared for the gateway integration thread.')
    add('TurnChanged',thread='firmware',state='completed')
    add('PendingChanged',thread='firmware',count=1)
    add('AskOpened',id='demo-question',thread='firmware',kind='input',tool='',detail='Which host interface should the gateway handoff target?',options='')
    msg('bt-mochi','user','What is ready across the sensor and gateway projects?')
    msg('bt-mochi','assistant','Sensor rev C has its power and layout reviews back. Tofu owns the review handoff; Pixel maintains the project dashboard. The gateway BLE handoff is ready for the next session.')
    add('SpaceSet',bot='pixel',spec=json.dumps({'title':'Rev C control desk','blocks':[{'type':'heading','text':'Sensor node / rev C'},{'type':'row','blocks':[{'type':'stat','label':'Review checks','value':'2 / 2','hint':'Power + layout'},{'type':'stat','label':'Handoffs','value':'Ready','hint':'Sensor → gateway'}]},{'type':'progress','label':'Review completion','value':1},{'type':'table','head':['Work','Harness','State'],'rows':[['Power budget','Claude','Reviewed'],['Layout','Grok','Reviewed'],['BLE handoff','Muse','Ready']]},{'type':'text','text':'A persistent project desk, kept by Pixel. Demo data for the README.'},{'type':'badge','text':'Ready for integration','tone':'ok'}]}))
    # Page is an ordinary bot-authored Space webapp, never a replacement app UI.
    html=(OUT/'source/space.html').read_text()
    add('SpacePageSet',bot='pixel',html=html)
    add('HookSet',id='demo-review-hook',bot='tofu',name='Review handoffs',revoked=False)
    # All mail and bodies are fictional. Canonical versioned body journal.
    maildir=home/'mail'; maildir.mkdir()
    for i,(subject,sender,body,want) in enumerate([
        ('Rev C power review','bot:mochi','Power and layout reviews are back. Please keep the sensor review handoff together for the gateway integration.','act'),
        ('Layout review complete','bot:pixel','Antenna keepout and connector orientation checked. The control desk has the latest review state.','fyi'),
        ('Gateway handoff','bot:mochi','The BLE handoff is ready. Link the sensor findings before the next integration session.','fyi')]):
        cid='demo-mail-'+str(i); mid='demo-message-'+str(i); at=now-90+i
        conv={'id':cid,'subject':subject,'kind':'dm','parts':[sender,'bot:tofu'],'project':'sensor','tags':['rev-c'],'made':at,'last':at,'n':1}
        mail={'id':mid,'conv':cid,'from':sender,'to':'bot:tofu','kind':'message','ref':'','want':want,'head':body,'size':len(body),'at':at}
        add('ConvSet',id=cid,subject=subject,kind='dm',parts='\n'.join(conv['parts']),project='sensor',tags='rev-c')
        add('MailPosted',**{k:v for k,v in mail.items() if k!='at'})
        (maildir/(cid+'.jsonl')).write_text(json.dumps({'v':1,'conv':conv,'mail':mail,'text':body})+'\n')
    eventtext=''.join(json.dumps(e)+'\n' for e in events)
    (home/'events.jsonl').write_text(eventtext)
    (OUT/'source/demo-events.jsonl').write_text(eventtext)
    (OUT/'source/demo-mail.json').write_text(json.dumps([json.loads(p.read_text()) for p in sorted(maildir.glob('*.jsonl'))],indent=2)+'\n')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
    display=next(n for n in range(90,100) if not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists())
    deny=home/'deny-bin'; deny.mkdir()
    for name in ['codex','claude','grok','muse','opencode']:
        stub=deny/name
        stub.write_text('#!/bin/sh\nprintf "%s %s\\n" "'+name+'" "$1" >> "'+str(home/'blocked-agents.log')+'"\nexit 125\n')
        stub.chmod(0o755)
    env={**{k:v for k,v in os.environ.items() if k in ['LANG','LC_ALL','TZ','LD_LIBRARY_PATH']},'HOME':str(home),'PATH':str(deny)+':/usr/bin:/bin','CODEX_HOME':str(home/'codex'),'CLAUDE_CONFIG_DIR':str(home/'claude'),'XDG_CONFIG_HOME':str(home/'config'),'XDG_CACHE_HOME':str(home/'cache'),'DISPLAY':f':{display}','BACKPLANE_HOME':str(home),'BACKPLANE_SNAP':str(home/'frame.ppm'),'BACKPLANE_NO_UPDATE':'1','BACKPLANE_PEERS':'','BACKPLANE_ADOPT':'0','BACKPLANE_KICAD_CLI':'/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli'}
    procs=[]
    try:
        log=(home/'xvfb.log').open('w')
        procs.append(subprocess.Popen([XVFB,f':{display}','-screen','0','3200x2000x24','-nolisten','tcp'],stdout=log,stderr=log))
        time.sleep(.5)
        log=(home/'app.log').open('w')
        procs.append(subprocess.Popen([str(APP),'--home',str(home),'--port',str(port),'--no-tailscale','--web','/home/h/.local/share/backplane/web'],env=env,stdout=log,stderr=log))
        time.sleep(5)
        def poke(*args): subprocess.run([XPOKE,*map(str,args)],env=env,check=True)
        poke('size',1600,1000); poke('key','Escape'); time.sleep(2)
        Image.open(home/'frame.ppm').save(OUT/'raw/native-initial.png')
        poke('click',100,386); time.sleep(2)
        Image.open(home/'frame.ppm').save(OUT/'raw/native-catbot.png')
        poke('click',455,62); time.sleep(2)
        Image.open(home/'frame.ppm').save(OUT/'raw/native-mailroom.png')
        poke('click',460,298); time.sleep(2)
        Image.open(home/'frame.ppm').save(OUT/'raw/native-mail-conversation.png')
        poke('size',3200,2000); time.sleep(3)
        Image.open(home/'frame.ppm').save(OUT/'raw/native-mailroom-hires.png')
        result=subprocess.run([BUN,str(OUT/'source/capture-web.mjs'),str(port),str(home)],env=env,text=True,capture_output=True)
        (OUT/'source/browser-log.txt').write_text(result.stdout+'\n'+result.stderr)
        print(result.stdout,result.stderr,flush=True)
        audit=[json.loads(l) for l in (home/'events.jsonl').read_text().splitlines()][len(events):]
        forbidden=[e for e in audit if (e.get('$') in ['SessionSet','TurnUsage'] or e.get('$')=='TurnChanged' and e.get('state')=='running' or e.get('$')=='MessagePosted' and e.get('role')=='assistant')]
        (OUT/'source/runtime-audit.json').write_text(json.dumps({'appended_events':audit,'forbidden_agent_events':forbidden,'blocked_agent_invocations':(home/'blocked-agents.log').read_text() if (home/'blocked-agents.log').exists() else ''},indent=2)+'\n')
        if forbidden: raise RuntimeError('Agent activity detected; discard captures')
        manifest={'app':str(APP),'app_sha256':hashlib.sha256(APP.read_bytes()).hexdigest(),'installed_commit_label':Path('/home/h/.local/share/backplane/COMMIT').read_text().strip(),'installed_version_label':Path('/home/h/.local/share/backplane/VERSION').read_text().strip(),'captured_at':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'home':str(home),'port':port,'display':display,'native_viewports':[[1600,1000],[3200,2000]],'browser_viewport':[1000,720],'browser_device_scale_factor':2,'events_sha256':hashlib.sha256(eventtext.encode()).hexdigest(),'browser_exit_code':result.returncode,'web_assets':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in Path('/home/h/.local/share/backplane/web').iterdir() if f.suffix in ['.js','.css','.html']},'demo_only':True}
        (OUT/'source/capture.json').write_text(json.dumps(manifest,indent=2)+'\n')
        (OUT/'source/app-log.txt').write_text((home/'app.log').read_text())
        print(json.dumps(manifest),flush=True)
        if result.returncode: raise RuntimeError('Browser capture failed')
    finally:
        for p in reversed(procs):
            p.terminate()
            try: p.wait(timeout=5)
            except subprocess.TimeoutExpired: p.kill(); p.wait()
if __name__=='__main__':main()
