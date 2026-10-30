#!/usr/bin/env python3
"""One coordinated, cached gate. No pushes, workflows, releases or installs."""
import argparse
import base64
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import math
import re
import shutil
import subprocess
import sys
import time
import zlib

ROOT = Path(__file__).resolve().parent.parent
CACHE = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'backplane-bend'


def run(argv, **kwargs):
    return subprocess.run(argv, cwd=ROOT, check=True, **kwargs)


def output(argv, **kwargs):
    return subprocess.check_output(argv, cwd=ROOT, text=True, **kwargs).strip()


def base_key(name, roots, files):
    env = {**os.environ, 'BP_KEY_ROOTS': ' '.join(roots)} if roots else os.environ
    return output(['scripts/inputs-key.sh', 'local-ci-v2:' + name, *sorted(set(files))], env=env)


def key(name, roots, files):
    # The existing key covers the compiler, its libraries and transitive effects.
    # Extra runners/drivers are explicit: a changed JS test cannot reuse a pass.
    try:
        base = base_key(name, roots, files)
        tools = [output(['node','--version']), output(['bun','--revision']), sys.version, sys.platform, resource.getrlimit(resource.RLIMIT_STACK)]
        environment = {k:v for k,v in os.environ.items() if k.startswith(('BUN_','BEND_','LC_','CHECK_','TEST_')) or k in ['BENDTT','CC','NODE_OPTIONS','LANG','TZ']}
        return hashlib.sha256(json.dumps([base, tools, environment], sort_keys=True).encode()).hexdigest()
    except subprocess.CalledProcessError:
        return None  # fail closed: run, never reuse an incomplete key


def gates():
    # Boat intentionally excludes logs. They are diagnostics, not build inputs;
    # tracked Kotlin crash logs must not make the remote source receipt unreadable.
    files = sorted(f for f in set(output(['git', 'ls-files', '--cached', '--others', '--exclude-standard']).splitlines()) if not f.endswith('.log'))
    def under(*prefixes):
        return [f for f in files if f.startswith(prefixes)]
    scripts = under('scripts/')
    tests = under('test/')
    checker = ['scripts/check.sh','scripts/bendok.sh','scripts/check-manifest.py','scripts/i18n-gen.py','scripts/inputs-key.sh','scripts/bend-closure.py','scripts/local-phase.py'] + under('i18n/')
    test_scripts = [f for f in scripts if not f.startswith('scripts/local')]
    test_roots = [f for f in files if f.startswith('test/') and f.endswith('_test.bend')]
    if os.environ.get('TEST_FULL') != '1':
        test_roots = [line.split('#',1)[0].strip() for line in (ROOT/'scripts/fast-tests.txt').read_text().splitlines()]
        test_roots = [line for line in test_roots if line]
    return [
        ('small', [], under('scripts/', 'test/tools/', 'desktop/', '.github/', 'assets/', 'deploy/') + ['desktop/package-lock.json','LICENSE'],
         [sys.executable, 'scripts/local-small.py']),
        ('check', [f for f in files if f.endswith('.bend') and f != 'PROOF.bend'], checker,
         ['scripts/check.sh', 'check']),
        ('proof', ['PROOF.bend'], checker, ['scripts/check.sh', 'proof']),
        ('build-smoke', ['src/server/main.bend', 'src/web/app.bend', 'src/mobile/app.bend', 'test/wire/wire.bend'], scripts + tests + under('src/web/', 'src/mobile/', 'mobile/', 'desktop/', 'assets/', 'deploy/') + ['LICENSE'],
         [sys.executable, 'scripts/local.py', 'build', '--smoke']),
        ('tests', test_roots, test_scripts + [f for f in tests if not f.endswith('.bend')] + under('deploy/') + [f for f in under('src/', 'tools/') if not f.endswith('.bend')],
         ['scripts/test.sh'] if os.environ.get('TEST_FULL') == '1' else ['timeout','120','scripts/test.sh']),
    ]


def local_phase(name, cmd, folder, cap):
    admit = Path(os.environ.get('BP_ADMIT', Path.home()/'Documents/i2cjak/bp-coord/bin/bp-admit.sh'))
    # --no-wait checks and reserves under the existing admission ledger lock.
    # No race between a memory preflight and an indefinitely queued request.
    if not admit.is_file() or '--no-wait)' not in admit.read_text():
        return 75, {'backend':'local', 'reason':'admission wrapper lacks --no-wait'}
    receipt = folder/(name+'.resources.json')
    env = {**os.environ, 'BP_LOCAL_CI_PHASE':'1', 'BACKPLANE_MEM_GB':str(cap)}
    if name == 'tests': env['TEST_TIMINGS_OUT'] = str(folder/'test-timings.txt')
    with (folder/(name+'.log')).open('a') as log:
        r = subprocess.run([str(admit),'--no-wait','--priority','--mem',str(cap),'--cores','2','--',
            sys.executable,'scripts/local-phase.py','--scope','--record',str(receipt),'--',*cmd],
            cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
    data = json.loads(receipt.read_text()) if receipt.is_file() else {}
    return r.returncode, {**data,'backend':'local','cap_gib':cap}


def boat_phase(name, gate, roots, files, folder, extra=None):
    spec = {'name':name,'gate':gate,'roots':roots,'files':sorted(set(files)),
            'base':base_key(gate,roots,files), 'check_shards':os.environ.get('CHECK_SHARDS','1'), **(extra or {})}
    encoded = base64.urlsafe_b64encode(zlib.compress(json.dumps(spec).encode())).decode()
    logpath = folder/(gate+'.boat.log')
    with logpath.open('w') as log:
        full = os.environ.get('TEST_FULL') == '1'
        size = 'default' if name=='focused' or (name=='tests' and not full) else 'large'
        seconds = '3600' if name=='tests' and full else '120' if name=='tests' else '900'
        command = ['scripts/boat-job.sh','--size',size,'--wait','0','--timeout',seconds,'--artifact','build/local-remote','--']
        if full and name=='tests':command += ['env','TEST_FULL=1']
        r = subprocess.run([*command,'python3','scripts/local-phase.py','--boat',encoded],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
    matches = re.findall(r'logs and artifacts in (\S+)',logpath.read_text())
    artifact = Path(matches[-1])/'artifacts/local-remote'/name if matches else None
    receipt = artifact/'phase.json' if artifact else None
    data = json.loads(receipt.read_text()) if receipt and receipt.is_file() else {}
    if r.returncode == 0 and (data.get('base')!=spec['base'] or data.get('rc')!=0):
        return 3, {'backend':'boat','reason':'missing or mismatched phase receipt'}, None
    if receipt and receipt.is_file(): shutil.copy2(receipt,folder/(gate+'.boat.resources.json'))
    return r.returncode, {**data,'backend':'boat','artifact':str(artifact) if artifact else None}, artifact


def execute_phase(name, roots, files, cmd, folder):
    # Initial ceilings have explicit provenance, not a claim of actual usage:
    # focused: observed 4.82 GiB; check: recorded OOM at10; proof: recorded >8;
    # emit: dev.sh's prior ~8 GiB; tests: two measured ~5 GiB processes.
    defaults = {'small':6,'check':16,'proof':12,'build-smoke':12,'tests':12 if os.environ.get('TEST_FULL')=='1' else 6}
    peaks = CACHE/'local-phase-peaks.json'
    try: history = json.loads(peaks.read_text())
    except (OSError,ValueError): history = {}
    history_name = name+(':full' if os.environ.get('TEST_FULL')=='1' else ':fast') if name=='tests' else name
    cap = math.ceil(history[history_name]*1.2/1073741824) if history_name in history else defaults[name]
    local = None
    warm = False
    if name == 'build-smoke':
        module = importlib.util.spec_from_file_location('local_build', ROOT/'scripts/local.py')
        local = importlib.util.module_from_spec(module);module.loader.exec_module(local)
        # Emit and bridge reuse omit those compilers. Web can still be cold:
        # the latest local web pass reached6GiB aggregate. Keep20% margin,
        # and track warm measurements separately from the cold server peak.
        warm = local.warm_build(os.environ)
        if warm: cap = math.ceil(history.get('build-smoke:warm',6*1073741824)*1.2/1073741824)
    forced = name in os.environ.get('BP_LOCAL_CI_BOAT','').split(',')
    rc, data = (75,{}) if forced else local_phase(name,cmd,folder,max(2,cap))
    if (rc in [75,-15,-9,137,143] and not data.get('seconds')):
        print(name+': local capacity unavailable; using Boat now',flush=True)
        if name == 'small':
            # Keep cheap dependency/Node guards local, offload only Bend.
            rc, guards = local_phase(name,[*cmd,'--guards-only'],folder,2)
            if rc: return rc,guards
            rc, focused, _ = boat_phase('focused',name,roots,files,folder)
            return rc,{'backend':'local+boat','guards':guards,'focused':focused}
        if name == 'build-smoke':
            sha = output(['git','rev-parse','HEAD'])
            version = '0.12.2-dev.local.'+sha[:9]+'.'+format(int(time.time()),'x')
            rc, remote, artifact = boat_phase('payload',name,roots,files,folder,{'commit':sha,'version':version,'bridge_needed':not local.phone_bridge_cached(os.environ)})
            if rc: return rc,remote
            rc, data = local_phase(name,[*cmd,'--payload',str(artifact)],folder,6)
            data['codegen'] = remote
        else:
            rc, data, _ = boat_phase(name,name,roots,files,folder)
    observed = data.get('aggregate_peak_bytes')
    if not observed and name in ['check','proof'] and data.get('peak_rss_kib'):
        # These commands run a single checker at a time; allow 64 MiB for
        # the Python/shell helpers in addition to the measured process peak.
        observed = data['peak_rss_kib']*1024 + 64*1024*1024
    if rc == 0 and observed:
        history[history_name] = max(history.get(history_name,0),observed)
        if warm: history['build-smoke:warm'] = max(history.get('build-smoke:warm',6*1073741824),observed)
        temp = peaks.with_suffix('.tmp');temp.write_text(json.dumps(history));temp.replace(peaks)
    return rc,data


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--dry-run', action='store_true')
    p.add_argument('--full', action='store_true', help='Ignore recorded passes')
    p.add_argument('--build-only', action='store_true', help='Stage and qualify without starting source gates')
    p.add_argument('--runtime', help=argparse.SUPPRESS)
    p.add_argument('--only',choices=['small','check','proof','build-smoke','tests'],help='Continue only an unfinished phase of this batch')
    a = p.parse_args()
    os.chdir(ROOT)
    os.environ.update(PYTHONDONTWRITEBYTECODE='1', BEND_NO_TELEMETRY='1', BACKPLANE_TELEMETRY='off',
                      BACKPLANE_TELEMETRY_BAKE='0', BACKPLANE_GOOGLE_BAKE='0',
                      CHECK_JOBS='1', CHECK_SHARDS=os.environ.get('CHECK_SHARDS','1'), CHECK_FALLBACK='0', TEST_JOBS='2', TEST_SHARD_COUNT='1', BUN_JSC_forceRAMSize='4294967296')
    CACHE.mkdir(parents=True, exist_ok=True)
    # One controller machine-wide, including across worktrees. Fail promptly;
    # never enqueue multiple admission requests or cancel another owner's work.
    with (CACHE / 'local-ci.lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print('A combined local CI run already owns this machine.', file=sys.stderr); return 3
        return execute(a)


def execute(a):
    if a.full: os.environ['BP_LOCAL_CI_FULL'] = '1'
    folder = ROOT / 'build/local-ci' / time.strftime('%Y%m%d-%H%M%S')
    records = CACHE / 'local-ci-passes'
    rows = []
    if not a.dry_run: folder.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    for name, roots, files, cmd in gates():
        if a.only and name != a.only: continue
        if a.build_only and name != 'build-smoke': continue
        if name == 'build-smoke' and a.runtime: cmd += ['--runtime',a.runtime]
        k = key(name, roots, files)
        record = records / (str(k) + '.json')
        try: prior = json.loads(record.read_text())
        except (OSError, ValueError): prior = {}
        # Recreate and inspect a stage every time. The emit/web/runtime caches
        # reuse valid inputs underneath; a pass is not proof an artifact exists.
        reused = name != 'build-smoke' and not a.full and k and prior.get('key') == k and prior.get('gate') == name
        print(f'{name}: {"reused" if reused else "run"} ({k or "unkeyed"})', flush=True)
        if a.dry_run: continue
        t = time.monotonic()
        row = {'gate': name, 'key': k, 'state': 'reused' if reused else 'running'}
        if name=='tests':row.update(test_files=roots,profile='full' if os.environ.get('TEST_FULL')=='1' else 'fast',budget_seconds=None if os.environ.get('TEST_FULL')=='1' else 120)
        if not reused:
            rc, metrics = execute_phase(name,roots,files,cmd,folder)
            incomplete = rc < 0 or rc in (70,75,76,77,124) or rc >= 128
            row.update(rc=rc,resources=metrics,state='passed' if rc == 0 else 'incomplete' if incomplete else 'failed')
            if rc == 0 and k and key(name, roots, files) == k:
                records.mkdir(parents=True, exist_ok=True)
                temp = record.with_suffix('.' + str(os.getpid()) + '.tmp')
                temp.write_text(json.dumps({'key': k, 'gate': name, 'commit': output(['git', 'rev-parse', 'HEAD'])}))
                temp.replace(record)
        if k and key(name, roots, files) != k: row['state'] = 'incomplete'
        row['seconds'] = round(time.monotonic() - t, 3)
        rows.append(row)
        result = {'seconds': round(time.monotonic()-start, 3), 'gates': rows,
                  'state': 'failed' if row['state'] == 'failed' else 'incomplete'}
        (folder / 'RESULT.json').write_text(json.dumps(result, indent=2) + '\n')
        if row['state'] in ('failed','incomplete'):
            print(f'{row["state"]}: {folder / (name + ".log")}'); return 1 if row['state'] == 'failed' else 3
    if not a.dry_run:
        result['state'] = 'passed'
        (folder / 'RESULT.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    sys.exit(main())
