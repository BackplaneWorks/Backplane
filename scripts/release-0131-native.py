#!/usr/bin/env python3
"""Single-run native packaging using existing producers and ordinary smoke."""
import hashlib,json,os,pathlib,subprocess,sys,tarfile,urllib.request
S=pathlib.Path.cwd(); O=S/'build/release-0131'; O.mkdir(parents=True)
target=os.environ['TARGET']; commit=os.environ['BACKPLANE_FROZEN_COMMIT']
def run(*args,env=None):subprocess.run(list(map(str,args)),check=True,env=env)
def sha(p):
 with pathlib.Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==commit
assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],text=True).strip()
u=os.environ['COMPONENTS_URL'].replace('{target}',target); assert u.startswith('https://')
expected=json.loads(os.environ['COMPONENTS_SHA'])[target]
a=O/'components.tar.gz'
with urllib.request.urlopen(u,timeout=120) as response,a.open('wb') as f:
 while block:=response.read(1024*1024):f.write(block)
assert sha(a)==expected
with tarfile.open(a) as tar:tar.extractall(O/'inputs',filter='fully_trusted')
services=list((S/'build/native-input').rglob('SERVICE.json')); assert len(services)==1
service=services[0];meta=json.loads(service.read_text());tree=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],text=True).strip()
assert (meta['sourceCommit'],meta['sourceTree'],meta['version'],meta['target'])==(commit,tree,'0.13.1',target)
run('node',S/'desktop/restore-modes.cjs',service,service.parent/'payload')
assert (S/'desktop/node_modules/ws/package.json').is_file(), 'npm ci must run in native shell before Python packaging'
for name,directory in [('SHELL.json','shell-payload'),('ELECTRON.json','payload')]:
 run('node',S/'desktop/restore-modes.cjs',O/'inputs/components'/target/name,O/'inputs/components'/target/directory)
run(sys.executable,S/'desktop/freeze-target.py',S,O/'inputs/components'/target,service,O/'freeze')
run('node',S/'desktop/produce-release.cjs','stage','--request',O/'freeze/stage.request.json','--output',O/'stage')
env=dict(os.environ)
if target=='windows-x64':
 keys=[k for k in env if k.lower()=='systemroot'];assert len(keys)==1
 value=env.pop(keys[0]);env['SystemRoot']=value
run('node',S/'desktop/release-ascii-startup.cjs',O/'stage',S,O/'evidence-ascii/RESULT.json',env=env)
cmd=['node',str(S/'desktop/test/release-smoke.cjs'),'--stage',str(O/'stage'),'--out',str(O/'evidence')]
if target.startswith('linux-'):
 run('unshare','--user','--map-root-user','/bin/true')
 cmd=['xvfb-run','--auto-servernum','--server-args=-screen 0 1280x900x24 -nolisten tcp','timeout','--signal=TERM','--kill-after=10s','180s',*cmd]
run(*cmd,env=env)
run('node',S/'desktop/archive-after-smoke.cjs','--stage',O/'stage','--evidence',O/'evidence','--out',O/'acceptance','--dist',O/'dist')
(O/'RESULT.json').write_text(json.dumps({'status':'PASS','target':target,'commit':commit,'tree':tree,'version':'0.13.1','componentsSha256':sha(a),'nativeScriptSha256':sha(__file__),'scope':'Existing actual ASCII startup + ordinary Unicode desktop smoke + strict archive, no broad regression; unsigned portable policy'},indent=2)+'\n')
