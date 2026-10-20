#!/usr/bin/env python3
"""Exclusive Xvfb admission wrapper; only captured child PIDs are signalled."""
import datetime, json, os, resource, select, shutil, subprocess, sys
from pathlib import Path
root=Path(__file__).resolve().parents[2]
name=sys.argv[1]; binary=(root/sys.argv[2]).resolve()
out=root/'build/schematic-refresh'/name
out.mkdir(parents=True,exist_ok=False)
report={'start':datetime.datetime.now(datetime.timezone.utc).isoformat(),'affinity':sorted(os.sched_getaffinity(0)), 'display':':187','tcp_listen':False, 'pids':[]}
assert not Path('/tmp/.X11-unix/X187').exists()
assert not Path('/tmp/.X187-lock').exists()
rd,wr=os.pipe()
log=(out/'xvfb.log').open('w')
xvfb=os.environ.get('SCHEMATIC_REFRESH_XVFB') or shutil.which('Xvfb')
assert xvfb,'set SCHEMATIC_REFRESH_XVFB to the host Xvfb binary'
x=subprocess.Popen([xvfb,':187','-screen','0','800x600x24','-nolisten','tcp','-displayfd',str(wr)],pass_fds=[wr],stdout=log,stderr=log)
report['pids'].append(x.pid);os.close(wr)
try:
    ready,_,_=select.select([rd],[],[],8)
    assert ready and os.read(rd,100).strip()==b'187','Xvfb failed readiness'
    os.environ['DISPLAY']=':187'
    for n in range(2):
        p=subprocess.Popen([sys.executable,str(root/'test/tools/schematic_refresh_native.py'),str(binary),str(out/f'run{n}')]+(['--trace'] if n==1 else []),stdout=(out/f'run{n}.log').open('w'),stderr=subprocess.STDOUT)
        report['pids'].append(p.pid)
        code=p.wait(timeout=25)
        if code: raise RuntimeError(f'run{n} exit {code}: '+(out/f'run{n}.log').read_text()[-3000:])
finally:
    x.terminate();x.wait(timeout=5);log.close();os.close(rd)
    report['end']=datetime.datetime.now(datetime.timezone.utc).isoformat()
    report['children_peak_rss_kib']=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    report['pids_absent']={str(p):not Path(f'/proc/{p}').exists() for p in report['pids']}
    report['socket_absent']=not Path('/tmp/.X11-unix/X187').exists()
    (out/'batch.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
