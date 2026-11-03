"""One actual proof shard in an isolated Boat VM, never a full proof/app build."""
import fcntl,hashlib,json,os,subprocess,sys
from pathlib import Path
assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX')=='1' and os.environ.get('BACKPLANE_HEAVY_OFFLOADED')=='1'
name=sys.argv[1];assert name in ['025','026']
root=Path.cwd();folder=root/'build/ci-speed/proof-shards/parallel-32-v3'
identity=hashlib.sha256((folder/'manifest.json').read_bytes()).hexdigest()
assert identity=='01118e14dc6b8a01f430528f153800ce926a041b81ccb5cb557dd45ca0c68d7e'
# This VM has one executor; take the actual shared native inode, no retries.
lock=Path('/tmp/bp-bend.lock').open('a+')
fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
fd=lock.fileno();os.set_inheritable(fd,True)
env={**os.environ,'BACKPLANE_HEAVY_LOCK_FD':str(fd),'PYTHONDONTWRITEBYTECODE':'1'}
result=folder/'receipts'/(name+'.json')
r=subprocess.run([sys.executable,'scripts/proof-shards.py','run','--root',str(root),'--out',str(folder),'--shard',name,'--expect',identity,'--result',str(result)],env=env,pass_fds=(fd,))
out=root/'build/ci-packet/proof-shards';out.mkdir(parents=True,exist_ok=True)
if result.is_file():(out/(name+'.json')).write_bytes(result.read_bytes())
if r.returncode:raise SystemExit(r.returncode)
# Independent typed false theorem: the proof body, not a parser error, must fail.
if name=='025':
 bad=out/'false-theorem.bend';bad.write_text('import Base\ndef false_theorem() -> {0n == 1n : Nat}:\n  {==}\n')
 control=subprocess.run(['bend',str(bad),'--check-only'],env=env,text=True,capture_output=True,timeout=20,pass_fds=(fd,))
 text=control.stdout+control.stderr;(out/'false-theorem.txt').write_text(text)
 assert control.returncode==1 and 'Location: false_theorem' in text and '- expected : 0n' in text and '- observed : 1n' in text,text
 print('ok typed false theorem rejected at proof body',flush=True)
print('ok actual shard',name,flush=True)
