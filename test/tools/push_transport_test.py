"""Real Bend/effect/child-IO path with a synthetic curl, never a network send."""
from pathlib import Path
import os,subprocess,tempfile,json,stat
root=Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='bp-push-fixture-')as tmp:
    base=Path(tmp);bindir=base/'bin';bindir.mkdir();out=base/'delivery';out.mkdir()
    fake=bindir/'curl'
    fake.write_text('''#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
args=sys.argv[1:]
assert args[args.index('--retry')+1]=='2'
assert args[args.index('--retry-max-time')+1]=='20'
assert '--retry-all-errors' not in args
cfg=Path(args[args.index('-K')+1]);values={}
for line in cfg.read_text().splitlines():
 k,_,v=line.partition(' = ')
 if k in ['output','data-binary','url']:values[k]=json.loads(v)
assert values['url'].startswith('https://synthetic.invalid/')
assert Path(values['data-binary'][1:]).read_text()=='{}'
log=Path(os.environ['PUSH_FIXTURE_LOG']);n=len(log.read_text().splitlines())if log.exists()else 0
code='400'if n<2 else '503';reason='TopicDisallowed'if code=='400'else'ServiceUnavailable'
Path(values['output']).write_text(json.dumps({'reason':reason}))
with log.open('a')as f:f.write(json.dumps({'job':n+1,'code':code,'configName':cfg.name})+'\\n')
print(code,end='')
''');fake.chmod(0o700)
    log=base/'calls.jsonl';env={**os.environ,'PATH':str(bindir)+os.pathsep+os.environ['PATH'],'PUSH_FIXTURE_DIR':str(out),'PUSH_FIXTURE_LOG':str(log),'BEND_NO_TELEMETRY':'1'}
    run=subprocess.run(['bend','test/push_transport_test.bend'],cwd=root,env=env,capture_output=True,text=True,timeout=40)
    print(run.stdout,end='');print(run.stderr,end='')
    assert run.returncode==0,(run.returncode,run.stderr)
    assert run.stdout.count('ok ')==5 and not any(l.startswith('FAIL')for l in run.stdout.splitlines())
    rows=[json.loads(x)for x in log.read_text().splitlines()];assert [r['code']for r in rows]==['400','400','503']
    assert len({r['configName']for r in rows})==3
    assert list(out.iterdir())==[], 'private curl files leaked'
    print('ok real effect and synthetic child transport sent exactly three jobs, no secret file or retry loop retained')
