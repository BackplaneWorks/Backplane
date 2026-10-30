#!/usr/bin/env python3
"""Qualify one staged local server, serially, with isolated test homes."""
import importlib.util
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('local_build', ROOT/'scripts/local.py')
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)


def inputs():
    files = subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],cwd=ROOT,text=True).splitlines()
    # Runtime gates also emit Bend fixtures (effects, phone payloads, smoke).
    # Their source and imported fixture helpers must invalidate a cached pass.
    named = sorted(f for f in files if f.startswith(('scripts/', 'test/', 'desktop/', 'src/mobile/', 'src/web/', 'deploy/')) or f == 'LICENSE')
    key = subprocess.check_output(['scripts/inputs-key.sh','local-runtime-v1',*named],cwd=ROOT,text=True,
        env={**os.environ,'BP_KEY_ROOTS':'src/server/main.bend src/mobile/app.bend test/wire/wire.bend'}).strip()
    versions = [subprocess.check_output(cmd,text=True).strip() for cmd in [['bun','--revision'],['node','--version']]]
    environment = {k:v for k,v in os.environ.items() if k.startswith(('BUN_','BEND_','LC_')) or k in ['BENDTT','CC','NODE_OPTIONS','LANG','TZ']}
    return local.hashlib.sha256(json.dumps([key,versions,sys.version,sys.platform,resource.getrlimit(resource.RLIMIT_STACK),environment],sort_keys=True).encode()).hexdigest()


def main():
    stage = Path(sys.argv[1]).resolve()
    info = json.loads((stage/'LOCAL.json').read_text())
    if not info.get('smoked'): raise SystemExit('UI acceptance must pass before runtime qualification')
    local.validate_stage(stage,info)
    before = inputs()
    cache = Path(os.environ.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'backplane-bend/local-runtime-passes'
    record = cache/(before+'.json')
    try: prior = json.loads(record.read_text())
    except (OSError,ValueError): prior = {}
    reuse = os.environ.get('BP_LOCAL_CI_FULL') != '1' and prior.get('key') == before and prior.get('state') == 'passed'
    logs = stage/'runtime-evidence';logs.mkdir()
    timings = []
    def run(name,cmd,extra=None):
        start = time.monotonic()
        print('runtime: '+name,flush=True)
        with (logs/(name+'.log')).open('w') as log:
            result = subprocess.run(cmd,cwd=ROOT,env={**os.environ,**(extra or {})},stdout=log,stderr=subprocess.STDOUT)
        timings.append({'gate':name,'seconds':round(time.monotonic()-start,3),'rc':result.returncode})
        (logs/'RESULT.json').write_text(json.dumps({'state':'running' if result.returncode==0 else 'failed','key':before,'reused_checks':bool(reuse),'gates':timings},indent=2)+'\n')
        if result.returncode:
            print((logs/(name+'.log')).read_text()[-12000:],file=sys.stderr)
            raise SystemExit(result.returncode if result.returncode>0 else 3)
    binary = str(stage/'payload/backplane-serve')
    web = str(stage/'payload/web')
    if reuse:
        # A new compiled artifact still has to start, identify itself and serve.
        run('startup',['scripts/js-smoke.sh',binary,web])
    else:
        run('wire',['bend','test/wire/index.html','-o','build/wire'])
        run('liveness-progress',['bun','test/tools/liveness_progress_test.ts'])
        run('lock-guard-liveness',['bun','test/tools/lock_guard_liveness_test.ts'])
        run('lock-helper-liveness',['bun','test/tools/lock_helper_liveness_test.ts'])
        run('work-intent-recovery',['bun','test/tools/work_intent_recovery_test.ts'])
        run('watch-scripts',['bun','test/tools/watch_scripts_test.ts'])
        run('liveness',['bun','test/tools/liveness_e2e.ts',binary,str(ROOT/'build/wire')])
        run('watched-work',['bun','test/tools/watch_e2e.ts',binary,str(ROOT/'build/wire')])
        run('locks',['bun','test/tools/locks_e2e.ts',binary,str(ROOT/'build/wire')])
        run('full-smoke',['scripts/js-smoke.sh',binary,web],{'BACKPLANE_SMOKE_FULL':'1'})
        run('mailtriage',['bun','test/tools/mailtriage_e2e.ts',binary,str(ROOT/'build/wire')])
        run('stack',['bun','test/tools/js_stack_test.ts',binary])
        run('bun-effects',['scripts/js-effects-bun.sh',str(logs/'effects')])
        run('bake',['bun','test/tools/js_bake_test.ts'])
        run('kicad-library-header',[sys.executable,'scripts/kicad-library-header.py',str(logs/'kicad-library.js')])
        run('kicad-library',['bun','test/tools/kicad_library_test.ts',str(logs/'kicad-library.js')])
        run('producer-guards',[sys.executable,'test/tools/service_release_test.py'])
        run('reconnect',['bun','test/tools/reconnect_test.ts'])
        run('heartbeat',['bun','test/tools/thin_heartbeat_e2e.ts',binary])
        for name in ['native_thin_rows_307265.mjs','native_thin_privacy_307265.mjs','native_thin_bridge_307265.mjs','mobile_readiness_test.ts','phone_flush_test.ts']:
            run(name.split('.')[0],['bun','test/tools/'+name])
    local.validate_stage(stage,info)
    if inputs() != before: raise SystemExit('Runtime inputs changed during qualification; no pass recorded')
    result = {'state':'passed','key':before,'reused_checks':bool(reuse),'gates':timings}
    (logs/'RESULT.json').write_text(json.dumps(result,indent=2)+'\n')
    cache.mkdir(parents=True,exist_ok=True)
    temp = record.with_suffix('.'+str(os.getpid())+'.tmp')
    temp.write_text(json.dumps({'key':before,'state':'passed'}));temp.replace(record)
    info.update(runtime_passed=True,runtime_key=before)
    (stage/'LOCAL.json').write_text(json.dumps(info,indent=2)+'\n')
    print(json.dumps(result),flush=True)


if __name__ == '__main__': main()
