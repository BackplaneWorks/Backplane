"""Fresh synthetic hub + one Chromium, in a loopback-only namespace.

python3 runtime.py start|stop|exec COMMAND...
All persistent state is under scratch/readme/run/C-features. No agents.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
import urllib.request

R = Path('<capture-root>')
RUN = R / 'run/C-features'
SRC = Path(__file__).resolve().parent
CHROME = '<ms-playwright>/chromium-1243/chrome-linux64/chrome'

def ns(args):
    return ['nsenter', '-t', (RUN/'ns.pid').read_text().strip(), '-U', '-n', '--preserve-credentials', *args]

def start():
    assert not (RUN/'ns.pid').exists(), 'Stop the previous run first'
    if (RUN/'home').exists(): shutil.rmtree(RUN/'home')
    for p in ['home', 'fakehome', 'project', 'agents', 'tmp', 'browser']:
        (RUN/p).mkdir(parents=True, exist_ok=True)
    events = [json.loads(s) for s in (SRC.parent/'demo-events.jsonl').read_text().splitlines()]
    for e in events:
        e['at'] = int(time.time())
        if e['$'] == 'ProjectCreated': e['root'] = str(RUN/'project')
        if e['$'] == 'RoomSet': e['members'] = 'demo-miso'
    for key, value in [('onboard.done','1'), ('restart.continue','off'), ('summary.scratch','Temporary threads and quick questions.')]:
        events.append({'$':'SettingSet','key':key,'value':value,'at':int(time.time())})
    seed = ''.join(json.dumps(e, ensure_ascii=False)+'\n' for e in events)
    (SRC/'demo-events.jsonl').write_text(seed)
    (RUN/'home/events.jsonl').write_text(seed)
    if (RUN/'project/review.txt').exists(): (RUN/'project/review.txt').chmod(0o644)
    shutil.copyfile(SRC.parent/'review-checklist.txt', RUN/'project/review.txt')
    (RUN/'project/review.txt').chmod(0o444)
    env = dict(os.environ)
    for key in ['DISPLAY','WAYLAND_DISPLAY','ELECTRON_RUN_AS_NODE']:
        env.pop(key,None)
    env.update(HOME=str(RUN/'fakehome'), TMPDIR=str(RUN/'tmp'),
        XDG_CONFIG_HOME=str(RUN/'fakehome/config'), XDG_CACHE_HOME=str(RUN/'fakehome/cache'),
        XDG_DATA_HOME=str(RUN/'fakehome/data'), BACKPLANE_AGENTS=str(RUN/'agents'),
        BACKPLANE_ORDER_SERVER='http://127.0.0.1:9', BACKPLANE_NO_UPDATE='1',
        BACKPLANE_KISTACK_SYNC='0', BACKPLANE_PEERS='', BACKPLANE_VAULT_KEYRING='file',
        BACKPLANE_HOME=str(RUN/'home'), SHELL='/bin/bash')
    with (RUN/'ns.log').open('w') as log:
        proc = subprocess.Popen(['unshare','-rn','sh','-c',f'ip link set lo up; echo $$ > {RUN}/ns.pid; exec sleep infinity'],env=env,stdout=log,stderr=log,start_new_session=True)
    for _ in range(50):
        if (RUN/'ns.pid').exists(): break
        time.sleep(.1)
    hub = [str(R/'app/backplane-serve'),'--headless','--foreground','--home',str(RUN/'home'),'--port','47830','--no-tailscale','--web',str(R/'app/web')]
    with (RUN/'hub.log').open('w') as log:
        proc = subprocess.Popen(ns(hub),env=env,stdout=log,stderr=log,start_new_session=True)
        (RUN/'hub.pid').write_text(str(proc.pid))
    for _ in range(100):
        check = subprocess.run(ns(['curl','-fs','-m','1','http://127.0.0.1:47830/hello']),capture_output=True)
        if check.returncode == 0:
            (SRC/'hello.cbor').write_bytes(check.stdout)
            break
        time.sleep(.5)
    else: raise RuntimeError('hub failed; inspect hub.log')
    browser = [CHROME,'--headless=new','--no-sandbox','--disable-gpu','--disable-background-networking','--disable-component-update','--no-first-run','--no-default-browser-check','--remote-debugging-port=47831','--remote-debugging-address=127.0.0.1',f'--user-data-dir={RUN}/browser','about:blank']
    with (RUN/'browser.log').open('w') as log:
        proc = subprocess.Popen(ns(browser),env=env,stdout=log,stderr=log,start_new_session=True)
        (RUN/'browser.pid').write_text(str(proc.pid))
    for _ in range(50):
        if subprocess.run(ns(['curl','-fs','-m','1','http://127.0.0.1:47831/json/version']),capture_output=True).returncode == 0: break
        time.sleep(.2)
    (SRC/'runtime.json').write_text(json.dumps({'date':'2026-11-01','build':(R/'app/VERSION').read_text().strip(),'commit':'36e6a1024','hub_sha256':hashlib.sha256((R/'app/backplane-serve').read_bytes()).hexdigest(),'home':str(RUN/'home'),'viewport':[1100,800],'deviceScaleFactor':2,'isolation':'unshare -rn, loopback only; empty agents directory'},indent=2)+'\n')
    print('Started isolated hub 47830 and Chromium CDP 47831')

def stop():
    for name, expected in [('browser',CHROME),('hub',str(R/'app/backplane-serve')),('ns','sleep')]:
        f=RUN/(name+'.pid')
        if not f.exists(): continue
        pid=int(f.read_text())
        cmd=Path(f'/proc/{pid}/cmdline')
        if cmd.exists() and expected.encode() in cmd.read_bytes():
            os.kill(pid,signal.SIGTERM)
            print('Stopped captured',name,'PID',pid)
        f.unlink()

if __name__=='__main__':
    if sys.argv[1]=='start': start()
    elif sys.argv[1]=='stop': stop()
    elif sys.argv[1]=='exec': sys.exit(subprocess.call(ns(sys.argv[2:])))
