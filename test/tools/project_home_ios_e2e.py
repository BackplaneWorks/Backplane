#!/usr/bin/env python3
"""Disposable Simulator, fresh private hub, synthetic projects and stand-ins only."""
import json, os, pathlib, shutil, socket, subprocess, sys, tempfile, time, urllib.request
from urllib.parse import quote
binary, app, fixtures, output = map(pathlib.Path, sys.argv[1:5])
root=pathlib.Path(tempfile.mkdtemp(prefix='bp-project-home-ios-'))
output.mkdir(parents=True,exist_ok=True)
child=sim=None
os.environ['DEVELOPER_DIR']='/Applications/Xcode.app/Contents/Developer'
def run(*args, **kw):
    return subprocess.run(args,check=True,capture_output=True,text=True,**kw).stdout.strip()
def until(fn,label,seconds=75):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        try:
            value=fn()
            if value:return value
        except (OSError,ValueError):pass
        time.sleep(.4)
    raise RuntimeError('Timeout: '+label)
def check(value,label):
    if not value:raise RuntimeError(label)
    print('ok '+label,flush=True)
def put(base,name,text):
    p=base/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
try:
    home=root/'hub';fake=root/'agents';home.mkdir();fake.mkdir()
    for cli in ['claude','codex','grok','muse','opencode','pi','omp','tailscale']:
        p=fake/cli;p.write_text('#!/bin/sh\nexit 127\n');p.chmod(0o700)
    at=int(time.time())-10
    rows=[]
    for pid,title in [('spec','Native spec home'),('software','Native software home'),('board','Native board home')]:
        p=root/pid;p.mkdir()
        put(p,'.backplane/intent.md','# Goal\nSynthetic native home qualification.\n\n# Parts\n\n# Notes\nPrivate fixture.\n')
        if pid=='spec':put(p,'docs/SPEC.md','# Specification\nPlanning stage with no board.\n')
        if pid=='software':put(p,'README.md','# README\nSoftware only.\n')
        if pid=='board':
            put(p,'.backplane.json',json.dumps({'pcb':'board.kicad_pcb'}))
            shutil.copyfile(fixtures/'pcb-bottom-silk/asymmetric.kicad_pcb',p/'board.kicad_pcb')
        rows.extend([{'$':'ProjectCreated','id':pid,'title':title,'root':str(p),'at':at},
                     {'$':'ThreadCreated','id':pid+'-thread','project':pid,'title':'Native discussion','env':'local','branch':'','worktree':str(p),'provider':'claude','model':'fixture','at':at}])
    for key,value in {'restart.continue':'off','telemetry.share':'off','fallback.on':'off','onboard.done':'1','onboard.uber':'1','sidebar.view':'projects'}.items():rows.append({'$':'SettingSet','key':key,'value':value,'at':at})
    log=home/'events.jsonl';log.write_text('\n'.join(map(json.dumps,rows))+'\n')
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    assert port!=3787
    origin='http://127.0.0.1:'+str(port)
    env={**os.environ,'PATH':str(fake)+':/usr/bin:/bin','HOME':str(root),'BACKPLANE_AGENTS':str(fake),'BACKPLANE_PEERS':'','BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_NO_UPDATE':'1','BACKPLANE_TELEMETRY':'off','BACKPLANE_ORDER_SERVER':'http://127.0.0.1:9'}
    hublog=(root/'hub.log').open('w')
    child=subprocess.Popen([str(binary),'--home',str(home),'--host','127.0.0.1','--port',str(port),'--foreground','--no-tailscale'],env=env,stdout=hublog,stderr=hublog)
    def api(endpoint,token='',method='GET'):
        assert child.poll() is None
        req=urllib.request.Request(origin+endpoint,method=method,headers={'Authorization':'Bearer '+token} if token else {})
        with urllib.request.urlopen(req,timeout=3) as r:return json.load(r)
    until(lambda: (home/'token').exists() and urllib.request.urlopen(origin+'/hello',timeout=2).status==200,'private hub')
    token=(home/'token').read_text().strip()
    runtime=next(x['identifier'] for x in json.loads(run('xcrun','simctl','list','runtimes','-j'))['runtimes'] if x['isAvailable'] and 'iOS' in x['name'])
    sim=run('xcrun','simctl','create','Backplane project home isolated','com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro',runtime)
    run('xcrun','simctl','boot',sim);run('xcrun','simctl','bootstatus',sim,'-b');run('xcrun','simctl','install',sim,str(app))
    ocr=root/'ocr.swift';ocr.write_text('''import Foundation
import Vision
import AppKit
let url=URL(fileURLWithPath:CommandLine.arguments[1])
let image=NSImage(contentsOf:url)!
var rect=CGRect(origin:.zero,size:image.size)
let cg=image.cgImage(forProposedRect:&rect,context:nil,hints:nil)!
let request=VNRecognizeTextRequest();request.recognitionLevel = .accurate
try VNImageRequestHandler(cgImage:cg).perform([request])
for item in request.results ?? [] {if let text=item.topCandidates(1).first?.string {print(text)}}
''')
    run('xcrun','swiftc',str(ocr),'-o',str(root/'ocr'))
    def screen(name):
        image=output/(name+'.png');run('xcrun','simctl','io',sim,'screenshot',str(image));return run(str(root/'ocr'),str(image))
    invite=api('/auth/invitations',token,'POST')
    pair='backplane://pair?url='+quote(invite['link'],safe='')
    base={**os.environ,'SIMCTL_CHILD_BACKPLANE_PAIR_LINK':pair,'SIMCTL_CHILD_BACKPLANE_SELECT':'project-home-spec'}
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env=base)
    until(lambda:'Specification' in screen('ios-spec-home'),'native specification home')
    check(True,'native specification home renders without a thread')
    acts='draft=Keep native draft;;home-rename={"kind":"project","id":"spec"};;rename-save=spec\x1fNative renamed spec;;look=spec;;look-set=project.icon.spec=cpu;;look='
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_SELECT':'project-home-spec','SIMCTL_CHILD_BACKPLANE_ACTS':acts})
    until(lambda:'Native renamed spec' in screen('ios-renamed-home'),'native inline rename')
    until(lambda:any(e.get('$')=='SettingSet' and e.get('key')=='project.icon.spec' and e.get('value')=='cpu' for e in map(json.loads,log.read_text().splitlines())),'native icon choice')
    check(True,'native rename and existing icon picker mutate the private canonical state')
    until(lambda:'Keep native draft' in screen('ios-renamed-home'),'native draft retained through rename and icon changes')
    check(True,'native home draft survives rename and icon changes')
    for pid,text in [('software','README'),('board','Native board home')]:
        run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_SELECT':'project-home-'+pid})
        until(lambda:text in screen('ios-'+pid+'-home'),'native '+pid+' home')
        check(True,'native '+pid+' home renders')
    check(sum(e.get('$')=='ThreadCreated' for e in map(json.loads,log.read_text().splitlines()))==3,'opening homes creates no threads')
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_SELECT':'project-home-spec','SIMCTL_CHILD_BACKPLANE_ACTS':'draft=One exact native home question;;send='})
    def sent():
        events=list(map(json.loads,log.read_text().splitlines()))
        threads=[e for e in events if e.get('$')=='ThreadCreated' and e.get('project')=='spec' and e.get('id','').startswith('home-')]
        return len(threads)==1 and sum(e.get('$')=='MessagePosted' and e.get('thread')==threads[0]['id'] and e.get('text')=='One exact native home question' for e in events)==1
    until(sent,'native exact first home message')
    check(True,'native home Send creates one real thread and its exact first message')
    (output/'receipt.json').write_text(json.dumps({'isolated':True,'root':str(root),'simulator':sim,'checks':7},indent=2))
finally:
    if sim:
        subprocess.run(['xcrun','simctl','shutdown',sim],capture_output=True)
        subprocess.run(['xcrun','simctl','delete',sim],capture_output=True)
    if child:
        child.terminate()
        try:child.wait(timeout=5)
        except subprocess.TimeoutExpired:child.kill();child.wait()
