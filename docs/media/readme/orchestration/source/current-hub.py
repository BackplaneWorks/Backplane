"""Disposable, loopback-only current-build screenshot hub. No real agents.
start / stop / run COMMAND...; all writes stay under scratch/readme.
"""
import json, os, signal, subprocess, sys, time
from pathlib import Path
R=Path('<capture-root>')
name=os.environ.get('README_B_RUN','B-publish')
assert name.startswith('B-') and '/' not in name
RUN=R/'run'/name
SRC=Path(__file__).resolve().parent
PORT=47820

def nscommand(pid):
    return ['nsenter','-t',str(pid),'-U','-n','-u','--preserve-credentials']

def stop():
    for name,expected in [('hub','--home'),('namespace','sleep')]:
        f=RUN/(name+'.pid')
        if not f.exists(): continue
        pid=int(f.read_text())
        proc=Path(f'/proc/{pid}/cmdline')
        if proc.exists():
            cmd=proc.read_bytes().replace(b'\0',b' ').decode()
            assert expected in cmd,(pid,cmd)
            if name=='hub': assert str(RUN/'home') in cmd
            os.kill(pid,signal.SIGTERM)
            print('Stopped',name,pid)
        f.unlink()

(R/'t-B').mkdir(exist_ok=True)
if sys.argv[1]=='stop': stop()
elif sys.argv[1]=='run':
    pid=int((RUN/'namespace.pid').read_text())
    env={k:v for k,v in os.environ.items() if k in ['PATH','LANG','LD_LIBRARY_PATH']}
    env.update(README_B_RUN=RUN.name,HOME=str(RUN/'fakehome'),TMPDIR=str(R/'t-B'),XDG_CACHE_HOME=str(RUN/'fakehome/.cache'),XDG_CONFIG_HOME=str(RUN/'fakehome/.config'))
    sys.exit(subprocess.call(nscommand(pid)+sys.argv[2:],env=env,cwd=RUN))
elif sys.argv[1]=='start':
    RUN.mkdir(parents=True,exist_ok=True)
    assert not (RUN/'namespace.pid').exists(),'Already running'
    home=RUN/'home'
    assert not home.exists(),'Use a new run folder for a new fixture'
    for d in ['home','home/mail','fakehome','tmp','agents','projects/sensor','projects/gateway']:
        (RUN/d).mkdir(parents=True,exist_ok=True)
    now=int(time.time())
    events=[json.loads(l) for l in (SRC/'demo-events.jsonl').read_text().splitlines()]
    events=[e for e in events if e['$'] not in ['PendingChanged','AskOpened']]
    for i,e in enumerate(events):
        e['at']=now-90+i
        if e['$']=='ProjectCreated': e['root']=str(RUN/'projects'/e['id'])
        if e['$']=='SpacePageSet': e['html']=(SRC/'space.html').read_text()
    events.extend([{'$':'ProjectCreated','id':'scratch','title':'Scratch','root':str(home/'scratch'),'at':now-30},{'$':'ThreadCreated','id':'notes','project':'scratch','title':'Review notes','env':'local','branch':'','worktree':'','provider':'codex','model':'gpt-6.1-sol','at':now-29},{'$':'MessagePosted','thread':'notes','msg':'demo-notes','role':'assistant','text':'Public README demo notes.','at':now-28},{'$':'TurnChanged','thread':'notes','state':'completed','at':now-27}])
    for k,v in [('order.enabled','off'),('telemetry.enabled','off')]:
        events.insert(0,{'$':'SettingSet','key':k,'value':v,'at':now-100})
    tasks={e['task'] for e in events if e['$']=='TaskDelegated'}
    assert tasks=={e['task'] for e in events if e['$']=='TaskDelivered'}
    assert all(e['state']=='done' for e in events if e['$']=='TaskStateSet')
    assert not any(e['$']=='TurnChanged' and e['state']=='running' for e in events)
    text=''.join(json.dumps(e,ensure_ascii=False)+'\n' for e in events)
    (home/'events.jsonl').write_text(text)
    (SRC/'current-events.jsonl').write_text(text)
    (SRC/'current-seed.json').write_text(json.dumps({'events':len(events),'port':PORT,'run':str(RUN),'build':'0.13.2-dev.20261101.1606','commit':'36e6a102411eac368f9ad458fb23167225786d27'},indent=2)+'\n')
    for row in json.loads((SRC/'demo-mail.json').read_text()):
        row['mail']['at']=now-30
        row['conv']['made']=row['conv']['last']=now-30
        (home/'mail'/(row['conv']['id']+'.jsonl')).write_text(json.dumps(row)+'\n')
    for name in ['claude','codex','grok','muse','opencode','pi','omp','agy_acp_server']:
        p=RUN/'agents'/name
        p.write_text('#!/bin/sh\nprintf "%s\\n" "'+name+' $1" >> "'+str(RUN/'blocked-agents.log')+'"\nexit 125\n')
        p.chmod(0o755)
    t=RUN/'agents/tailscale'
    t.write_text("#!/bin/sh\nprintf '%s\\n' '{\"Self\":{\"HostName\":\"desktop\",\"TailscaleIPs\":[]},\"Peer\":{}}'\n")
    t.chmod(0o755)
    env={'PATH':str(RUN/'agents')+':/usr/bin:/bin','HOME':str(RUN/'fakehome'),'TMPDIR':str(RUN/'tmp'),'LANG':'C.UTF-8','XDG_CONFIG_HOME':str(RUN/'fakehome/.config'),'XDG_CACHE_HOME':str(RUN/'fakehome/.cache'),'XDG_DATA_HOME':str(RUN/'fakehome/.local/share'),'XDG_STATE_HOME':str(RUN/'fakehome/.local/state'),'CODEX_HOME':str(RUN/'fakehome/codex'),'CLAUDE_CONFIG_DIR':str(RUN/'fakehome/claude'),'BACKPLANE_NAME':'desktop','BACKPLANE_HOME':str(home),'BACKPLANE_AGENTS':str(RUN/'agents'),'BACKPLANE_NO_UPDATE':'1','BACKPLANE_ORDER_SERVER':'http://127.0.0.1:9','BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_PEERS':'','BACKPLANE_ADOPT':'0','BACKPLANE_VAULT_KEYRING':'file'}
    ns=subprocess.Popen(['unshare','-rnu','sh','-c','hostname desktop; ip link set lo up; exec sleep infinity'],env=env,cwd=RUN,start_new_session=True,stdout=(RUN/'namespace.log').open('w'),stderr=subprocess.STDOUT)
    (RUN/'namespace.pid').write_text(str(ns.pid))
    time.sleep(.3)
    assert ns.poll() is None
    cmd=[str(R/'app/backplane-serve'),'--headless','--foreground','--home',str(home),'--port',str(PORT),'--no-tailscale','--web',str(R/'app/web')]
    hub=subprocess.Popen(nscommand(ns.pid)+cmd,env=env,cwd=RUN,start_new_session=True,stdout=(RUN/'hub.log').open('w'),stderr=subprocess.STDOUT)
    (RUN/'hub.pid').write_text(str(hub.pid))
    for _ in range(100):
        if hub.poll() is not None: raise RuntimeError((RUN/'hub.log').read_text())
        p=subprocess.run(nscommand(ns.pid)+['curl','-fs','-m','1',f'http://127.0.0.1:{PORT}/hello'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
        if p.returncode==0:
            (SRC/'current-hello.cbor').write_bytes(p.stdout)
            print('Ready',PORT,'namespace',ns.pid,'hub',hub.pid,'seed events',len(events))
            break
        time.sleep(.2)
    else: raise RuntimeError('Hub did not become ready')
