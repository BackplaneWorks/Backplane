#!/usr/bin/env python3
"""Build, preview and safely apply a local Backplane without a release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
PREFIX = Path(os.environ.get('BACKPLANE_PREFIX', Path.home() / '.local/share/backplane'))
HOME = Path(os.environ.get('BACKPLANE_INSTALL_HOME', Path.home() / '.backplane-bend'))
STORE = Path.home() / '.local/share/backplane-local'


def run(argv, **kwargs):
    return subprocess.run(list(map(str, argv)), cwd=ROOT, check=True, **kwargs)


def profiled(argv, record, **kwargs):
    return run([sys.executable,'scripts/local-phase.py','--record',record,'--',*argv],**kwargs)


def digest(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def dependencies():
    key = digest(ROOT/'desktop/package-lock.json') + digest(ROOT/'desktop/package.json')
    stamp = ROOT/'desktop/node_modules/.backplane-inputs'
    if stamp.is_file() and stamp.read_text() == key:
        return
    run(['npm', 'ci', '--ignore-scripts', '--no-audit', '--no-fund', '--prefix', 'desktop'])
    stamp.write_text(key)


def payload_hashes(stage):
    return {str(p.relative_to(stage)): digest(p) for p in sorted((stage/'payload').rglob('*')) if p.is_file()}


def validate_stage(stage, info):
    if payload_hashes(stage) != info.get('files'):
        raise SystemExit('Stage payload changed after the smoke')
    manifest = stage/'desktop/MANIFEST.json'
    if digest(manifest) != info.get('desktop_manifest_sha256'):
        raise SystemExit('Desktop manifest changed after the smoke')
    for name, expected in json.loads(manifest.read_text())['files'].items():
        p = stage/'desktop'/name
        if isinstance(expected, dict):
            st = p.stat()
            if not p.is_symlink() or os.readlink(p) != expected['symlink'] or st.st_uid or st.st_gid or st.st_mode & 0o7777 != 0o4755:
                raise SystemExit('Shared OS sandbox helper changed')
            expected = expected['sha256']
        if digest(p) != expected: raise SystemExit('Desktop file changed: '+name)


def greeting():
    # Reuse the audited, bounded CBOR greeting parser. Never execute the installed hub.
    result = subprocess.check_output(['node', '-e',
        "fetch('http://127.0.0.1:3787/hello',{signal:AbortSignal.timeout(5000)}).then(r=>r.arrayBuffer()).then(b=>console.log(JSON.stringify(require('./desktop/service.cjs').hello(Buffer.from(b))))).catch(e=>{console.error(e.message);process.exit(1)})"], cwd=ROOT, text=True)
    return json.loads(result)


def runtime(given):
    candidates = [Path(given)] if given else [PREFIX / 'desktop/runtime'] + sorted(
        (Path.home() / '.local/share/backplane-electron/releases').glob('*/runtime'), reverse=True)
    for p in candidates:
        if (p / 'backplane-desktop').is_file() and (p / 'version').is_file() and (p / 'version').read_text().strip() == '44.9.2':
            return p.resolve()
    raise SystemExit('Supply --runtime DIR containing the pinned Electron 44.9.2 runtime')


def phone_bridge_inputs(env):
    named = ['scripts/build-mobile.sh','scripts/bridge-native.py','scripts/js-unmarshal.py','scripts/state-key.py','src/mobile/index.html','src/mobile/bridge.js']
    # The HTML bundle also reads JS imports shared with the browser; Bend's
    # import closure alone does not follow those JavaScript module edges.
    named += sorted(str(p.relative_to(ROOT)) for folder in ['src/web','src/mobile'] for p in (ROOT/folder).glob('*.js'))
    bun = subprocess.check_output(['bun','--revision'],cwd=ROOT,text=True,env=env).strip()
    def key():
        return subprocess.check_output(['scripts/inputs-key.sh','local-phone-bridge-v2:'+bun,*sorted(set(named))], cwd=ROOT,
            env={**env,'BP_KEY_ROOTS':'src/mobile/app.bend'}, text=True).strip()
    return key


def phone_bridge_cached(env):
    before = phone_bridge_inputs(env)()
    cache = Path(os.environ.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'backplane-bend/phone-bridge'/before
    receipt = cache.with_suffix('.ok')
    return cache.is_file() and receipt.is_file() and receipt.read_text().strip() == digest(cache)


def warm_build(env):
    key = subprocess.check_output(['scripts/build-js.sh','--key','src/server/main.bend'],cwd=ROOT,env=env,text=True).strip()
    cache = Path(env.get('BACKPLANE_EMIT_CACHE') or Path(env.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'backplane-bend/emit-js')
    emit, receipt = cache/(key+'.js'), cache/(key+'.js.ok')
    return emit.is_file() and receipt.is_file() and receipt.read_text().strip() == str(emit.stat().st_size) and phone_bridge_cached(env)


def import_emit(prepared, remote, env):
    key = subprocess.check_output(['scripts/build-js.sh','--key','src/server/main.bend'],cwd=ROOT,env=env,text=True).strip()
    if remote['emit_key'] != key or digest(prepared/'emit.js') != remote['emit_sha256']:
        raise SystemExit('Transferred emit input/hash mismatch')
    if env.get('BACKPLANE_EMIT_CACHE') == '0': return
    cache = Path(env.get('BACKPLANE_EMIT_CACHE') or Path(env.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'backplane-bend/emit-js')
    cache.mkdir(parents=True,exist_ok=True)
    target = cache/(key+'.js')
    temp = target.with_suffix('.'+str(os.getpid())+'.tmp')
    shutil.copy2(prepared/'emit.js',temp);temp.replace(target)
    target.with_suffix('.js.ok').write_text(str(target.stat().st_size))


def phone_bridge(env, prepared=None):
    key = phone_bridge_inputs(env)
    before = key()
    cache = Path(os.environ.get('XDG_CACHE_HOME',Path.home()/'.cache'))/'backplane-bend/phone-bridge'/before
    receipt = cache.with_suffix('.ok')
    bridge = ROOT/'mobile/build/assets/bridge.js'
    if prepared and json.loads((prepared/'REMOTE.json').read_text()).get('phone_bridge_key'):
        meta = json.loads((prepared/'REMOTE.json').read_text())
        if meta['phone_bridge_key'] != before or digest(prepared/'bridge.js') != meta['bridge_sha256']:
            raise SystemExit('Transferred bridge input/hash mismatch')
        cache.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(prepared/'bridge.js',cache)
        receipt.write_text(digest(cache))
    valid = cache.is_file() and receipt.is_file() and receipt.read_text().strip() == digest(cache)
    if valid:
        bridge.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(cache,bridge)
        print('phone bridge reused ('+before+')',flush=True)
    else:
        run(['scripts/build-mobile.sh','--js'],env=env)
        if key() == before:
            cache.parent.mkdir(parents=True,exist_ok=True)
            temp = cache.with_suffix('.'+str(os.getpid())+'.tmp');shutil.copy2(bridge,temp);temp.replace(cache)
            temp = receipt.with_suffix('.'+str(os.getpid())+'.tmp');temp.write_text(digest(cache));temp.replace(receipt)
    return before


def build(args):
    if sys.platform != 'linux': raise SystemExit('Local desktop staging currently requires Linux')
    stage = ROOT / 'build/local' / time.strftime('%Y%m%d-%H%M%S')
    stage.mkdir(parents=True, exist_ok=False)
    timings = {}
    measurements = stage/'measurements';measurements.mkdir()
    env = {**os.environ, 'PYTHONDONTWRITEBYTECODE':'1', 'BEND_NO_TELEMETRY': '1', 'BACKPLANE_TELEMETRY_BAKE': '0',
           'BACKPLANE_TELEMETRY': 'off', 'BACKPLANE_GOOGLE_BAKE': '0', 'BACKPLANE_NO_UPDATE': '1',
           'BACKPLANE_SKIP_CHECK': '1', 'BACKPLANE_DEV_SERVICE_ONLY': '1', 'BACKPLANE_MEM_GB': '12'}
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    stable = json.loads((ROOT/'desktop/package.json').read_text())['version'].split('.')
    next_version = '.'.join([*stable[:2], str(int(stable[2])+1)])
    env['BACKPLANE_VERSION_STAMP'] = next_version + '-dev.local.' + sha[:9] + '.' + format(int(time.time()), 'x')
    if len(env['BACKPLANE_VERSION_STAMP']) > 40: raise SystemExit('Local version exceeds the compiler stamp slot')
    t = time.monotonic()
    prepared = args.payload.resolve() if args.payload else None
    if prepared:
        remote = json.loads((prepared/'REMOTE.json').read_text())
        if remote['commit'] != sha or payload_hashes(prepared) != remote['files']:
            raise SystemExit('Transferred payload source/hash mismatch')
        if remote.get('emit_key'): import_emit(prepared,remote,env)
        env['BACKPLANE_VERSION_STAMP'] = remote['version']
        shutil.copytree(prepared/'payload',stage/'payload',symlinks=True)
        for name in ['metrics.json','web-metrics.json','phase.json']:
            if (prepared/name).is_file(): shutil.copy2(prepared/name,measurements/('boat-'+name))
    else:
        profiled(['scripts/dev.sh', 'stage', stage / 'payload'], measurements/'service-web.json',env=env)
        for source,name in [(ROOT/'build/js/backplane-serve/metrics.json','codegen-compile.json'),(ROOT/'build/web-metrics.json','web.json')]:
            if source.is_file():shutil.copy2(source,measurements/name)
    timings['service_web_seconds'] = round(time.monotonic()-t, 3)
    t = time.monotonic()
    dependencies()
    # Only dependencies, never Electron downloads or an OS sandbox-policy change.
    env.update(BACKPLANE_ELECTRON_DIST=str(runtime(args.runtime)), BACKPLANE_DESKTOP_RESIDENT='1',
               BACKPLANE_DESKTOP_BROWSER='1', BACKPLANE_SOURCE_COMMIT=sha)
    profiled(['node', 'desktop/stage-linux.cjs', stage / 'payload/backplane-serve',
         stage / 'payload/web', env['BACKPLANE_VERSION_STAMP'], stage / 'desktop'], measurements/'desktop-packaging.json',env=env)
    timings['desktop_seconds'] = round(time.monotonic()-t, 3)
    record = {'stage': str(stage), 'commit': sha, 'version': env['BACKPLANE_VERSION_STAMP'],
              'service_sha256': digest(stage / 'payload/backplane-serve'), 'timings': timings,
              'files':payload_hashes(stage), 'desktop_manifest_sha256':digest(stage/'desktop/MANIFEST.json'),
              'smoked': False, 'qualification': 'local test build; no release or publication'}
    (stage / 'LOCAL.json').write_text(json.dumps(record, indent=2)+'\n')
    if args.smoke:
        t = time.monotonic()
        record['phone_bridge_key'] = phone_bridge(env,prepared)
        timings['phone_bridge_seconds'] = round(time.monotonic()-t, 3)
        sdk = Path(env.get('ANDROID_HOME') or '/opt/android-sdk')
        if (sdk/'platforms/android-35/android.jar').is_file():
            android_env = {**env, 'ANDROID_HOME':str(sdk), 'JAVA_HOME':env.get('JAVA_HOME') or '/usr/lib/jvm/java-17-openjdk-amd64'}
            t = time.monotonic()
            run(['mobile/android/gradlew','-p','mobile/android','--no-daemon','--max-workers=2',
                 '-Dorg.gradle.jvmargs=-Xmx2g -Dfile.encoding=UTF-8', '-Pkotlin.compiler.execution.strategy=in-process',
                 ':app:compileDebugKotlin'],env=android_env)
            timings['android_compile_seconds'] = round(time.monotonic()-t,3)
            record['android'] = 'debug Kotlin compiled; no release, publication or device claim'
        else: record['android'] = 'not compiled: Android API35 platform unavailable; no SDK/admin changes made'
        record['ios'] = 'not compiled on Linux: requires a Mac'
        t = time.monotonic()
        env.update(BACKPLANE_SERVICE=str(stage / 'payload/backplane-serve'), BACKPLANE_WEB=str(stage / 'payload/web'),
                   BACKPLANE_SERVICE_VERSION=record['version'], BACKPLANE_LOCAL_STAGE=str(stage))
        run(['node', 'desktop/test/local-ui.cjs'], env=env)
        timings['smoke_seconds'] = round(time.monotonic()-t, 3)
        t = time.monotonic()
        run(['node','desktop/test/bub-links.cjs'],env=env)
        timings['bub_links_seconds'] = round(time.monotonic()-t,3)
        t = time.monotonic()
        run(['node','desktop/test/bot-replies.cjs'],env=env)
        timings['bot_replies_seconds'] = round(time.monotonic()-t,3)
        t = time.monotonic()
        run(['node','desktop/test/catalog-images.cjs'],env=env)
        timings['catalog_images_seconds'] = round(time.monotonic()-t,3)
        record['smoked'] = True
        (stage / 'LOCAL.json').write_text(json.dumps(record, indent=2)+'\n')
        # Same serial controller, after the shared build and UI acceptance.
        # No second server build or independently scheduled suite.
        t = time.monotonic()
        run([sys.executable, 'scripts/local-runtime.py', stage], env=env)
        record = json.loads((stage/'LOCAL.json').read_text())
        timings['runtime_seconds'] = round(time.monotonic()-t, 3)
        record['timings'] = timings
        (stage / 'LOCAL.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(record, indent=2), flush=True)
    (ROOT / 'build/local/LATEST').write_text(str(stage)+'\n')


def request_deploy(stage, note):
    body = {'jsonrpc':'2.0', 'id':1, 'method':'tools/call', 'params':{
        'name':'deploy_restart', 'arguments':{'stage':str(stage), 'note':note}}}
    token = (HOME / 'token').read_text().strip()
    owner = os.environ.get('BACKPLANE_THREAD_ID', os.environ.get('BACKPLANE_THREAD', 'deploy'))
    if not all(c.isalnum() or c in '-_' for c in owner): raise SystemExit('Invalid deploy requester')
    req = urllib.request.Request('http://127.0.0.1:3787/mcp/' + owner, data=json.dumps(body).encode(),
          headers={'Content-Type':'application/json', 'Accept':'application/json', 'Authorization':'Bearer '+token})
    with urllib.request.urlopen(req, timeout=10) as r: answer = r.read().decode()
    if 'scheduled' not in answer: raise SystemExit('Hub did not schedule the deploy; installed files unchanged: '+answer)
    print('Safe deploy queued. After the harness reports restart, run activate for this stage.', flush=True)


def apply(stage):
    started = time.monotonic()
    info = json.loads((stage / 'LOCAL.json').read_text())
    if not info['smoked']: raise SystemExit('This stage has no passing local UI smoke')
    if not info.get('runtime_passed'): raise SystemExit('This stage has no passing combined runtime gate')
    if digest(stage / 'payload/backplane-serve') != info['service_sha256']: raise SystemExit('Stage service changed')
    validate_stage(stage, info)
    hello = greeting()
    installed = STORE / stage.name
    if installed.exists(): raise SystemExit('This stage was already prepared; inspect it instead of overwriting rollback')
    installed.mkdir(parents=True)
    shutil.copytree(stage, installed / 'stage', symlinks=True)
    validate_stage(installed/'stage', info)
    backup = installed / 'rollback'; backup.mkdir()
    for name in ['backplane', 'backplane-serve', 'LICENSE']:
        if (PREFIX / name).exists(): shutil.copy2(PREFIX / name, backup / name)
    shutil.copytree(PREFIX / 'web', backup / 'web')
    launcher = Path.home() / '.local/bin/backplane-open'
    if launcher.exists(): shutil.copy2(launcher, installed / 'previous-launcher')
    desktop_entry = Path.home()/'.local/share/applications/backplane.desktop'
    if desktop_entry.exists(): shutil.copy2(desktop_entry,installed/'previous-desktop-entry')
    else: (installed/'previous-desktop-entry-absent').touch()
    (installed / 'BEFORE.json').write_text(json.dumps({'hello':hello, 'prefix':str(PREFIX), 'home':str(HOME)}, indent=2)+'\n')
    # The hub consumes this copy; retain the review stage and complete rollback.
    shutil.copytree(installed / 'stage/payload', installed / 'pending')
    print('Rollback:', installed, flush=True)
    request_time = time.time()
    request_deploy(installed / 'pending', 'Local Backplane polish; preserve headless supervision and privacy settings')
    (installed/'DEPLOY.json').write_text(json.dumps({'requested_unix':request_time,
        'prepare_and_queue_seconds':round(time.monotonic()-started,3), 'state':'scheduled'},indent=2)+'\n')


def activate(stage):
    installed = STORE / stage.name
    info = json.loads((installed / 'stage/LOCAL.json').read_text())
    hello = greeting()
    if hello.get('backplane') != info['version'] or hello.get('commit') != info['commit']:
        raise SystemExit('New hub is not ready at the expected version/commit; launcher unchanged')
    validate_stage(installed/'stage',info)
    launcher = Path.home() / '.local/bin/backplane-open'
    env = {'BACKPLANE_HOME':str(HOME), 'BACKPLANE_TELEMETRY':'off', 'BACKPLANE_NO_UPDATE':'1',
           'BACKPLANE_DESKTOP_BROWSER':'1', 'BACKPLANE_SERVICE':str(PREFIX/'backplane-serve'),
           'BACKPLANE_WEB':str(PREFIX/'web'), 'BACKPLANE_SERVICE_VERSION':info['version']}
    text = '#!/bin/sh\nset -eu\n' + ''.join('export '+k+'='+shlex.quote(v)+'\n' for k,v in env.items())
    text += 'exec '+shlex.quote(str(installed/'stage/desktop/runtime/backplane-desktop'))+' "$@"\n'
    temp = launcher.with_suffix('.local-new');temp.write_text(text);temp.chmod(0o755);temp.replace(launcher)
    desktop_entry = Path.home()/'.local/share/applications/backplane.desktop'
    if desktop_entry.exists():
        lines = desktop_entry.read_text().splitlines()
    else:
        executable = str(launcher).replace('\\','\\\\').replace('"','\\"').replace('$','\\$').replace('`','\\`').replace('%','%%')
        lines = ['[Desktop Entry]','Type=Application','Name=Backplane','Exec="'+executable+'"','Terminal=false','Categories=Development;']
    # Preserve desktop actions and unrelated fields in an existing launcher.
    section = False; updated = []
    for line in lines:
        if line.startswith('['):
            if section: updated += ['Icon='+str(installed/'stage/desktop/runtime/resources/app/backplane.png'),'StartupWMClass=backplane']
            section = line == '[Desktop Entry]'
        if not (section and line.startswith(('Icon=','StartupWMClass='))): updated.append(line)
    if section: updated += ['Icon='+str(installed/'stage/desktop/runtime/resources/app/backplane.png'),'StartupWMClass=backplane']
    desktop_entry.parent.mkdir(parents=True,exist_ok=True)
    temp = desktop_entry.with_suffix('.local-new');temp.write_text('\n'.join(updated)+'\n');temp.replace(desktop_entry)
    receipt = installed/'DEPLOY.json'
    if receipt.is_file():
        deployed = json.loads(receipt.read_text())
        deployed.update(state='ready-observed',ready_observed_unix=time.time(),
            request_to_ready_observed_upper_bound_seconds=round(time.time()-deployed['requested_unix'],3))
        receipt.write_text(json.dumps(deployed,indent=2)+'\n')
    print('Local desktop ready. Open Backplane again; the resident hub stays supervised. Rollback:', installed)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest='command', required=True)
    sub.add_parser('deps')
    b = sub.add_parser('build');b.add_argument('--runtime');b.add_argument('--smoke', action='store_true')
    b.add_argument('--admitted', action='store_true', help=argparse.SUPPRESS)
    b.add_argument('--payload',type=Path,help=argparse.SUPPRESS)
    for cmd in ['preview','apply','activate','rollback']:
        s = sub.add_parser(cmd);s.add_argument('stage', type=Path)
    a = p.parse_args();os.chdir(ROOT)
    if a.command == 'deps': dependencies();return 0
    if a.command == 'build':
        if not a.admitted and not os.environ.get('BP_LOCAL_CI_PHASE'):
            command = [sys.executable,'scripts/local-ci.py','--build-only']
            if a.runtime:command += ['--runtime',a.runtime]
            return subprocess.call(command,cwd=ROOT)
        build(a);return 0
    stage = a.stage.resolve()
    if a.command == 'apply': apply(stage)
    elif a.command == 'activate': activate(stage)
    elif a.command == 'preview':
        import tempfile
        env = {**os.environ,'BACKPLANE_HOME':tempfile.mkdtemp(prefix='backplane-local-preview-'),
               'BACKPLANE_TELEMETRY':'off', 'BACKPLANE_NO_UPDATE':'1','BACKPLANE_COPY':'1'}
        os.execve(str(stage/'desktop/backplane-electron'),[str(stage/'desktop/backplane-electron')],env)
    elif a.command == 'rollback':
        installed = STORE / stage.name
        request_deploy(installed/'rollback','Restore pre-local Backplane service and web')
        launcher = Path.home()/'.local/bin/backplane-open'
        if (installed/'previous-launcher').exists():
            temp=launcher.with_suffix('.rollback-new');shutil.copy2(installed/'previous-launcher',temp);temp.replace(launcher)
        desktop_entry = Path.home()/'.local/share/applications/backplane.desktop'
        if (installed/'previous-desktop-entry').exists():
            temp=desktop_entry.with_suffix('.rollback-new');shutil.copy2(installed/'previous-desktop-entry',temp);temp.replace(desktop_entry)
        elif (installed/'previous-desktop-entry-absent').exists(): desktop_entry.unlink(missing_ok=True)
    return 0


if __name__ == '__main__': sys.exit(main())
