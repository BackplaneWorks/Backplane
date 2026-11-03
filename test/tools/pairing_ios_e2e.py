#!/usr/bin/env python3
"""Native simulator smoke against a private hub. Requires a Mac and compiled app."""
import json, os, pathlib, shutil, subprocess, sys, tempfile, time, urllib.request, urllib.error
from urllib.parse import quote, urlparse
hub, app, bridge = map(pathlib.Path, sys.argv[1:4])
root = pathlib.Path(tempfile.mkdtemp(prefix='bp-pairing-ios-'))
sim = None
child = None
os.environ['DEVELOPER_DIR'] = '/Applications/Xcode.app/Contents/Developer'
def run(*args, **kw):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kw).stdout.strip()
def check(value, label):
    if not value: raise RuntimeError(label)
    print('ok ' + label, flush=True)
def until(fn, label, seconds=50):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            value = fn()
            if value: return value
        except (OSError, urllib.error.URLError): pass
        time.sleep(.4)
    raise RuntimeError('timeout: ' + label)
try:
    home, fake = root/'hub', root/'agents'
    home.mkdir(); fake.mkdir()
    for cli in ['claude','codex','grok','muse','opencode','pi','omp']:
        p=fake/cli; p.write_text('#!/bin/sh\nexit 127\n'); p.chmod(0o755)
    import socket
    with socket.socket() as s:
        s.bind(('127.0.0.1',0)); port=s.getsockname()[1]
    origin=f'http://127.0.0.1:{port}'
    env={**os.environ,'HOME':str(root),'BACKPLANE_AGENTS':str(fake),'BACKPLANE_KISTACK_SYNC':'0','BACKPLANE_NO_UPDATE':'1','BACKPLANE_TELEMETRY':'off','BACKPLANE_ORDER_SERVER':'http://127.0.0.1:9'}
    out=(root/'hub.log').open('w')
    child=subprocess.Popen(['/Users/h/.bun/bin/bun',str(hub),'--home',str(home),'--web',str(root),'--port',str(port),'--host','127.0.0.1','--no-tailscale'],env=env,stdout=out,stderr=out)
    def api(path, token=None, method='GET', headers=None):
        hs=headers or {}
        if token: hs={**hs,'Authorization':'Bearer '+token}
        req=urllib.request.Request(origin+path,method=method,headers=hs)
        with urllib.request.urlopen(req,timeout=8) as r: return json.load(r)
    until(lambda: (home/'token').exists() and urllib.request.urlopen(origin+'/hello',timeout=2).status==200,'hub ready')
    token=(home/'token').read_text().strip()
    runtimes=json.loads(run('xcrun','simctl','list','runtimes','-j'))['runtimes']
    runtime=next(x['identifier'] for x in runtimes if x['isAvailable'] and x['name'].startswith('iOS'))
    sim=run('xcrun','simctl','create','Backplane pairing 1293799','com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro',runtime)
    run('xcrun','simctl','boot',sim)
    run('xcrun','simctl','bootstatus',sim,'-b')
    shutil.copyfile(bridge,app/'bridge.js')
    run('xcrun','simctl','install',sim,str(app))
    run('xcrun','simctl','launch',sim,'dev.backplane.mobile')
    ocr=root/'ocr.swift'
    ocr.write_text('''import AppKit
import Vision
let image=NSImage(contentsOfFile:CommandLine.arguments[1])!
var rect=CGRect(origin:.zero,size:image.size)
let cg=image.cgImage(forProposedRect:&rect,context:nil,hints:nil)!
let request=VNRecognizeTextRequest()
try VNImageRequestHandler(cgImage:cg).perform([request])
for item in request.results ?? [] { if let text=item.topCandidates(1).first?.string { print(text) } }
''')
    def text():
        image=root/'screen.png'
        run('xcrun','simctl','io',sim,'screenshot',str(image))
        return run('xcrun','swift',str(ocr),str(image))
    until(lambda: 'Connect to your Backplane hub' in text().replace('\n',' '),'native first-run',90)
    check(True,'native iOS presents the full-screen first-run view')
    invite=api('/auth/invitations',token,'POST')
    link='backplane://pair?url='+quote(invite['link'],safe='')
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_PAIR_LINK':link})
    try:
        device=until(lambda: next(iter(api('/auth/devices',token)['devices']),None),'native invitation exchange',60)
    except RuntimeError:
        import re
        shown=text()
        shown=re.sub(r'[a-f0-9]{32}|[A-Z2-9]{8}', '[redacted]', shown)
        print('iOS status: '+shown.replace('\n',' / ')[:1200], flush=True)
        evidence=pathlib.Path('evidence');evidence.mkdir(exist_ok=True)
        shutil.copyfile(root/'screen.png',evidence/'ios-pairing-failure.png')
        raise
    check(bool(device),'native iOS invitation launch creates its individual session')
    broken=origin+'/pair#invite=invalid'
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_PAIR_LINK':broken})
    time.sleep(3)
    check(len([d for d in api('/auth/devices',token)['devices'] if not d['revoked']])==1,'malformed invitation preserves the native iOS pairing')
    api('/auth/revoke',token,'POST',{'X-Backplane-Device':device['id']})
    until(lambda:'Re-pair this device' in text().replace('\n',' '),'native re-pair screen',90)
    check(True,'native iOS presents full-screen re-pair after revocation')
    invite=api('/auth/invitations',token,'POST')
    run('xcrun','simctl','launch','--terminate-running-process',sim,'dev.backplane.mobile',env={**os.environ,'SIMCTL_CHILD_BACKPLANE_PAIR_LINK':'backplane://pair?url='+quote(invite['link'],safe='')})
    until(lambda:len([d for d in api('/auth/devices',token)['devices'] if not d['revoked']])==1,'native re-pair exchange',60)
    check(True,'native iOS re-pairs without resetting other device credentials')
    check(token not in (root/'hub.log').read_text(),'native fixture hub does not log its credential')
finally:
    if sim:
        subprocess.run(['xcrun','simctl','shutdown',sim],capture_output=True)
        subprocess.run(['xcrun','simctl','delete',sim],capture_output=True)
    if child:
        child.terminate()
        try:child.wait(timeout=10)
        except subprocess.TimeoutExpired:child.kill();child.wait()
    shutil.rmtree(root)
