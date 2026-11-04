"""Serial fresh-process measurements on Boat; retain raw logs and metrics."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import statistics
import subprocess
import time
import signal

root = Path('build/bend-vs-ts/runtime')
root.mkdir(parents=True, exist_ok=True)
env = {**os.environ, 'BUN_JSC_forceRAMSize':'4294967296', 'BUN_JSC_maxPerThreadStackUsage':'536870912'}
samples = []

def command(name, args, *, gc=False, timeout=300):
    start = time.monotonic()
    with (root/(name+'.stdout')).open('w') as stdout, (root/(name+'.stderr')).open('w') as stderr:
        e = {**env, **({'BUN_JSC_logGC':'1'} if gc else {})}
        proc = subprocess.Popen(args, stdout=stdout, stderr=stderr, env=e)
        def expired(signum, frame):
            raise TimeoutError(f'{name} exceeded {timeout}s')
        previous=signal.signal(signal.SIGALRM, expired)
        signal.alarm(timeout)
        try:
            _, status, usage = os.wait4(proc.pid, 0)
        except BaseException:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
            raise
        finally:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, previous)
        rc = os.waitstatus_to_exitcode(status)
        proc.returncode = rc
    result = dict(name=name, seconds=time.monotonic()-start, rc=rc, peak_rss_kib=usage.ru_maxrss,
                  cpu_user_s=usage.ru_utime, cpu_system_s=usage.ru_stime)
    (root/(name+'.resources.json')).write_text(json.dumps(result, indent=2)+'\n')
    if rc:
        raise RuntimeError(f'{name} failed ({rc}): '+(root/(name+'.stderr')).read_text()[-4000:])
    return (root/(name+'.stdout')).read_text(), (root/(name+'.stderr')).read_text(), result

hardware = dict(platform=platform.platform(), cpu=Path('/proc/cpuinfo').read_text().split('model name')[1].split('\n')[0].strip(' :'),
                cpus=os.cpu_count(), meminfo=Path('/proc/meminfo').read_text(), bun=subprocess.check_output(['bun','--version'],text=True).strip(),
                commit=os.environ.get('BENCH_SOURCE_REVISION','unknown'), fixture=json.loads(Path('test/perf/bend-vs-ts/input/manifest.json').read_text()))
(root/'hardware.json').write_text(json.dumps(hardware,indent=2)+'\n')
command('prepare',['bun','test/perf/bend-vs-ts/prepare.ts',str(root)],timeout=600)
for path, sizes in [('status',[500,1000]),('phase',[500,1000]),('broadcast',[725000,1450000,2900000]),('history',[1,2])]:
    for size in sizes:
        command(f'verify-{path}-{size}',['bun','test/perf/bend-vs-ts/runner.ts','verify',path,str(size)])
        modes=['bend','ts'] + (['bend-scan'] if path=='status' else ['ts-trie'] if path=='phase' else ['ts-query'] if path=='history' else [])
        # The raw emitter control isolates today's native primitive patches.
        if path in ('status','broadcast'): modes+=['bend-raw']
        repeats=50 if path in ('status','phase') else 4 if path=='history' else 1
        for trial in range(3):
            # Alternate order to reduce drift from host noise.
            for mode in modes if trial%2==0 else list(reversed(modes)):
                name=f'{path}-{size}-{mode}-{trial}'
                stdout, stderr, res=command(name,['bun','test/perf/bend-vs-ts/runner.ts',mode,path,str(size),str(repeats)],gc=True)
                data=json.loads(stdout.strip().splitlines()[-1])
                active=stderr.split('BENCH_BEGIN\n',1)[1].split('BENCH_WORK_END\n',1)[0]
                allocation=stderr.split('BENCH_BEGIN\n',1)[1].split('BENCH_END\n',1)[0]
                # JSC ca: allocation before collection; a: cumulative allocation
                # while collection is concurrent. Max a per cycle avoids counting
                # intermediate snapshots twice. The final explicit collection
                # closes the last interval, outside wallMs. A lower-bound estimate
                # (allocator granularity / late concurrent allocations remain).
                allocated=0
                for cycle in re.findall(r'\bSTART\b.*?\bEND\]',allocation,re.S):
                    ca=re.search(r'\bca=([0-9.]+)kb',cycle)
                    during=[float(a) for a in re.findall(r'(?<![a-z])a=([0-9.]+)kb',cycle)]
                    allocated+=1024*((float(ca[1]) if ca else 0)+max(during,default=0))
                data.update(resources=res, gc_pause_ms=sum(map(float,re.findall(r'(?<![a-z])p=([0-9.]+)ms',active))),
                            allocated_bytes_estimate=allocated,
                            gc_cycles=len(re.findall(r'\bSTART ',active)))
                samples.append(data)
                (root/'samples.json').write_text(json.dumps(samples,indent=2)+'\n')
                print(json.dumps({k:data[k] for k in ['mode','path','size','msPerOp','gc_pause_ms','allocated_bytes_estimate']}),flush=True)
        # Output hashes must agree among identical-representation outputs.
        group=[s for s in samples if s['path']==path and s['size']==size]
        if len({s['digest'] for s in group})!=1:
            raise RuntimeError(f'Non-identical outputs: {path} {size}')
command('tsc-noemit',['bun','test/perf/bend-vs-ts/node_modules/typescript/bin/tsc','--strict','--noEmit','--target','ES2022','--module','ESNext','--lib','ES2022','--skipLibCheck','test/perf/bend-vs-ts/equivalents.ts'])
command('tsc-emit',['bun','test/perf/bend-vs-ts/node_modules/typescript/bin/tsc','--strict','--target','ES2022','--module','ESNext','--lib','ES2022','--skipLibCheck','--outDir',str(root/'tsc'),'test/perf/bend-vs-ts/equivalents.ts'])
command('bun-build-ts',['bun','build','test/perf/bend-vs-ts/equivalents.ts','--target=bun','--outfile',str(root/'equivalents.js')])
command('bun-link-hub',['bun','build','--compile','--minify-whitespace','--minify-syntax','--keep-names','--no-compile-autoload-dotenv','--no-compile-autoload-bunfig','test/perf/bend-vs-ts/input/hub-original.js','--outfile',str(root/'hub-bench')])
# The executable is compile evidence only; it is deliberately never run.
(root/'hub-bench').unlink()
print('ALL OUTPUT CHECKS PASSED',flush=True)
