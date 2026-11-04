"""Bounded attempt to produce actual current-main hub bytes for native acceptance.

No retained older emission, source overlay, synthetic qualification or package
claim. Only run inside the approved Boat/GitHub resource scope.
"""
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time

out = Path('build/kicad-927-native'); out.mkdir(parents=True, exist_ok=True)
group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
def kernel():
    return {'memory_max':int((group/'memory.max').read_text()),
            'swap_max':int((group/'memory.swap.max').read_text()),
            'peak_bytes':int((group/'memory.peak').read_text()),
            'events':dict((k,int(v)) for k,v in (line.split() for line in (group/'memory.events').read_text().splitlines()))}
before=kernel()
assert 0 < before['memory_max'] <= 3_700_000_000 and before['swap_max']==0, before
command=['bend','src/server/main.bend','-o',str(out/'raw-hub.js')]
report={'source_commit_requested':sys.argv[1], 'command':command, 'timeout_seconds':180,
        'kernel_before':before,'packaged_current_main':False,'native_windows_checked':False,
        'source_sha256':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in
                         ['src/server/main.bend','src/server/kicad.bend','src/core/kicad_install.bend','src/server/effects/cad-install.js']}}
start=time.monotonic(); timedout=False
env=dict(os.environ,BEND_NO_TELEMETRY='1',BUN_JSC_forceRAMSize='4294967296',BUN_JSC_maxPerThreadStackUsage='536870912')
with (out/'hub-emission.txt').open('wb') as log:
    child=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
    try: code=child.wait(timeout=180)
    except subprocess.TimeoutExpired:
        timedout=True; os.killpg(child.pid,signal.SIGTERM)
        try: code=child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid,signal.SIGKILL);code=child.wait(timeout=5)
after=kernel();delta={k:after['events'].get(k,0)-before['events'].get(k,0) for k in after['events']}
raw=out/'raw-hub.js'
report.update(exit_code=code,seconds=round(time.monotonic()-start,3),timed_out=timedout,kernel_after=after,events_delta=delta,
              peak_child_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
              reason='cgroup_oom_kill' if delta.get('oom_kill') else 'timeout' if timedout else 'exit_'+str(code),
              state='EMITTED_NOT_PACKAGED' if code==0 and raw.is_file() and raw.stat().st_size else 'BLOCKED_NO_CURRENT_MAIN_PACKAGE')
if report['state']=='EMITTED_NOT_PACKAGED':report['raw_sha256']=hashlib.sha256(raw.read_bytes()).hexdigest()
(out/'RESULT.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2));print((out/'hub-emission.txt').read_text(errors='replace')[-5000:])
