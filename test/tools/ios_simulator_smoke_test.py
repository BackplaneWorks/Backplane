#!/usr/bin/env python3
"""Synthetic xcrun controls only; never starts a real simulator or compiler."""
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile

smoke = Path('test/tools/ios_simulator_smoke.py').resolve()
owned = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'
with tempfile.TemporaryDirectory(prefix='bp-simulator-smoke-control-') as folder:
    root = Path(folder); tools = root/'bin'; tools.mkdir()
    app = root/'Fixture.app'; app.mkdir()
    (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'fixture.smoke'}))
    (app/'bridge.js').write_text('synthetic bridge')
    fake = tools/'xcrun'
    fake.write_text('''#!/usr/bin/env python3
import json,os,signal,sys
from pathlib import Path
a=sys.argv[1:];mode=os.environ['SIM_SMOKE_MODE']
with open(os.environ['SIM_SMOKE_LOG'],'a') as log:log.write(json.dumps(a)+'\\n')
assert a[0]=='simctl';a=a[1:];op=a[0]
owned='bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'
if op=='list' and a[1]=='runtimes':
 print(json.dumps({'runtimes':[] if mode=='no-runtime' else [
 {'identifier':'com.apple.CoreSimulator.SimRuntime.iOS-99-0','version':'99.0','isAvailable':False},
 {'identifier':'com.apple.CoreSimulator.SimRuntime.tvOS-99-0','version':'99.0','isAvailable':True},
 {'identifier':'com.apple.CoreSimulator.SimRuntime.iOS-18-0','version':'18.0','isAvailable':True}]}))
elif op=='list':
 print(json.dumps({'devicetypes':[
 {'identifier':'fixture.ipad','name':'iPad'},
 {'identifier':'fixture.future','name':'iPhone future','minRuntimeVersionString':'26.0'},
 {'identifier':'fixture.phone','name':'iPhone fixture','minRuntimeVersionString':'18.0.0','maxRuntimeVersionString':'20.0'}]}))
elif op=='create':
 assert a[-2:] == ['fixture.phone','com.apple.CoreSimulator.SimRuntime.iOS-18-0'];print(owned)
else:
 assert a[1]==owned,'operation aimed at an unowned simulator'
 if op=='install' and mode=='install-fails':sys.exit(19)
 if op=='install' and mode=='signal':os.kill(os.getppid(),signal.SIGTERM)
 if op=='launch':print('fixture.smoke: 1234')
 if op=='spawn':print('- 0 other.service' if mode=='crash' else '1234 0 UIKitApplication:fixture.smoke')
 if op=='io':Path(a[-1]).write_bytes(b'\\x89PNG\\r\\n\\x1a\\nsynthetic')
''')
    fake.chmod(0o755)
    def run(mode):
        out=root/mode;log=root/(mode+'.jsonl')
        env=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ['PATH'],SIM_SMOKE_MODE=mode,SIM_SMOKE_LOG=str(log))
        result=subprocess.run(['python3',str(smoke),'--app',str(app),'--out',str(out),'--settle-seconds','0'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=20)
        report=json.loads((out/'result.json').read_text())
        calls=[json.loads(line)[1:] for line in log.read_text().splitlines()]
        return result,report,calls
    result,report,calls=run('healthy')
    assert result.returncode==0 and report['result']=='pass' and report['launchedPid']==1234
    assert calls[-2:]==[['shutdown',owned],['delete',owned]]
    print('ok enumerated runtime/device, owned launch/PID/screenshot and exact cleanup')
    for mode in ['install-fails','crash','signal']:
        result,report,calls=run(mode)
        assert result.returncode and report['result']=='fail'
        assert calls[-2:]==[['shutdown',owned],['delete',owned]]
        assert not any('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa' in call for call in calls)
        print('ok '+mode+' cannot pass and cleans only its newly owned simulator')
    result,report,calls=run('no-runtime')
    assert result.returncode and report['result']=='fail' and not any(call[0] in ['create','shutdown','delete'] for call in calls)
    print('ok missing available iOS runtime fails before any simulator ownership change')
