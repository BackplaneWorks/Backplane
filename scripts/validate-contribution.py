#!/usr/bin/env python3
"""Run a required validation lane and save exact source, commands and artifacts."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
os.chdir(ROOT)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('lane', choices=['pure', 'native', 'browser', 'phone', 'real-cad', 'build', 'emit'])
parser.add_argument('--out', type=Path, default=Path('build/validation'))
parser.add_argument('--binary', default='dist/backplane')
parser.add_argument('--build-receipt', type=Path, help='passing build/result.json for the consumed hub binary')
parser.add_argument('--prebuilt-receipt', type=Path, help='passing emit/result.json for BACKPLANE_PREBUILT=1')
args = parser.parse_args()
output_root = args.out.resolve()
if output_root == ROOT or output_root in ROOT.parents:
    parser.error('--out must not contain the source checkout')
args.build_receipt = (args.build_receipt or output_root/'build/result.json').resolve()
args.out = output_root / args.lane
args.out.mkdir(parents=True, exist_ok=True)

def capture(command):
    try:
        r = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20)
        return {'command': command, 'status': r.returncode, 'output': r.stdout.strip()}
    except (OSError, subprocess.TimeoutExpired) as e:
        return {'command': command, 'status': 'unavailable', 'output': str(e)}

def source():
    names = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard']).split(b'\0')
    h = hashlib.sha256()
    count = 0
    for name in sorted(set(filter(None, names))):
        path = ROOT / os.fsdecode(name)
        if path == output_root or output_root in path.parents:
            continue
        h.update(name + b'\0')
        if path.exists() or path.is_symlink():
            h.update((path.lstat().st_mode & 0o777).to_bytes(2, 'big'))
        if path.is_symlink():
            h.update(b'link\0' + os.fsencode(os.readlink(path)))
        elif path.is_file():
            h.update(path.read_bytes())
        else:
            h.update(b'<missing>')
        h.update(b'\0')
        count += 1
    status_command = ['git','status','--porcelain','--untracked-files=all','--','.']
    if output_root.is_relative_to(ROOT):
        status_command.append(':(top,literal,exclude)'+str(output_root.relative_to(ROOT)))
    return {'commit': subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
            'sha256': h.hexdigest(), 'files': count,
            'status': subprocess.check_output(status_command,text=True).splitlines()}

lanes = {
 'pure': [['scripts/check.sh'], ['scripts/test.sh']],
 'native': [
    ['scripts/build-app.sh','test/native/release_child.bend','build/release-child'],
    ['bun','test/tools/release_exports_e2e.ts','build/release-child'],
    ['bun','test/tools/release_transaction_e2e.ts','build/release-child'],
    ['scripts/build-app.sh','test/native/worktree_lifecycle_test.bend','build/worktree_lifecycle_test'],
    ['build/worktree_lifecycle_test'],
    ['scripts/build-app.sh','test/native/worktree_carry_test.bend','build/worktree_carry_test'],
    ['build/worktree_carry_test'],
    ['scripts/build-app.sh','test/native/checkpoint_ignored_test.bend','build/checkpoint_ignored_test'],
    ['build/checkpoint_ignored_test'],
    ['scripts/build-app.sh','test/native/kicad_platform.bend','build/kicad_platform'],
    ['bun','test/tools/kicad_platform_e2e.ts','build/kicad_platform'],
    ['scripts/build-app.sh','test/native/log_read.bend','build/log-read'],
    ['bun','test/tools/logread_e2e.ts',args.binary,'build/wire','build/log-read'],
    ['bend','test/fixtures/outbox/index.html','-o','build/outbox-client'],
    ['bun','test/tools/outbox_e2e.ts',args.binary,'build/outbox-client','mobile/build/assets/bridge.js'],
    ['bun','test/tools/retire_e2e.ts',args.binary,'build/wire'],
 ],
 'browser': [
    ['scripts/build-wire.sh'],
    ['bun','tools/browser/test/input.ts'],
    ['bun','test/tools/teach_e2e.ts',args.binary,'build/wire'],
    ['bun','test/tools/browser_auth_test.ts'],
 ],
 'phone': [
    ['scripts/build-mobile.sh','--js'],
    ['bun','test/tools/ios_recent_test.ts'],
    ['bend','test/tools/ios_recent_fixture.html','-o','build/ios-recent-fixture'],
    ['sh','-c','cat build/ios-recent-fixture/chunk-*.js > build/ios-recent-fixture.js'],
    ['env','BUN_JSC_useJIT=0','BUN_JSC_maxPerThreadStackUsage=4194304','bun','test/tools/ios_recent_bridge_test.ts','mobile/build/assets/bridge.js','build/ios-recent-fixture.js'],
    ['bun','test/tools/phone_live_test.ts','mobile/build/assets/bridge.js'],
 ],
 'real-cad': [
    ['python3','scripts/fetch-cad-fixture.py'],
    ['scripts/build-wire.sh'],
    ['bun','test/tools/review_e2e.ts',args.binary,'build/wire'],
    ['bun','test/tools/hist_e2e.ts',args.binary,'build/wire'],
 ],
 'build': [['scripts/build.sh']],
 'emit': [['scripts/build.sh', 'emit']],
}
report = {'schema':2,'lane':args.lane,'started':datetime.now(timezone.utc).isoformat(),
          'source':source(),'host':{'platform':platform.platform(),'machine':platform.machine()},
          'toolchain':[capture(c) for c in [['bend','version'],['bun','--version'],['clang','--version'],['git','--version'],['python3','--version']]],
          'flags':{k:os.environ.get(k) for k in ['CC','BACKPLANE_CFLAGS','BACKPLANE_LOW_MEM','BACKPLANE_JOBS','BACKPLANE_CPUS','BACKPLANE_PREBUILT','BACKPLANE_GOOGLE_BAKE','BACKPLANE_KICAD_CLI','BUN_JSC_forceRAMSize','BUN_JSC_useJIT','BACKPLANE_CHROME','BACKPLANE_TEST_BROWSERS']},
          'steps':[], 'artifacts':[], 'consumed':{}, 'provenance':{}, 'result':'running'}
report_path = args.out/'result.json'

def bend_version(toolchain):
    probes = [item for item in toolchain if isinstance(item, dict) and item.get('command') == ['bend', 'version']]
    probe = probes[0] if len(probes) == 1 else {}
    parsed = re.search(r'\b\d+\.\d+\.\d+(?:[+-][0-9A-Za-z.-]+)?\b', str(probe.get('output', '')))
    if probe.get('status') != 0 or not parsed:
        raise ValueError('Bend version probe must succeed with a version: bend version')
    return parsed.group(0)

def artifact(path, role=None):
    path = Path(path).resolve(strict=True)
    if not path.is_file():
        raise ValueError(f'not a regular file: {path}')
    try:
        display = str(path.relative_to(ROOT))
    except ValueError:
        display = str(path)
    result = {'path':display, 'resolved_path':str(path), 'bytes':path.stat().st_size,
              'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
    if role:
        result['role'] = role
    return result

def receipt(path, lane):
    path = Path(path).resolve(strict=True)
    raw = path.read_bytes()
    value = json.loads(raw)
    if not isinstance(value, dict) or value.get('schema') != 2 or value.get('lane') != lane or value.get('result') != 'pass':
        raise ValueError(f'{path}: a passing schema-2 {lane} receipt is required')
    if not isinstance(value.get('source'), dict) or not isinstance(value.get('source_after'), dict):
        raise ValueError(f'{path}: producer source identity is missing')
    identity = lambda item: (item.get('commit'), item.get('sha256'))
    if identity(value.get('source', {})) != identity(report['source']):
        raise ValueError(f'{path}: receipt source does not match current commit and source hash')
    if value.get('source_after') != value.get('source'):
        raise ValueError(f'{path}: producer source changed during validation')
    if not isinstance(value.get('steps'), list) or not value['steps'] or any(not isinstance(step, dict) or step.get('result') != 'pass' for step in value['steps']):
        raise ValueError(f'{path}: producer commands did not all pass')
    bend_version(value.get('toolchain', []))
    if not isinstance(value.get('artifacts'), list) or any(not isinstance(item, dict) or not isinstance(item.get('path'), str) or not isinstance(item.get('sha256'), str) for item in value['artifacts']):
        raise ValueError(f'{path}: producer artifact hashes are missing')
    return value, {'receipt':str(path), 'receipt_sha256':hashlib.sha256(raw).hexdigest(),
                   'source_commit':report['source']['commit'], 'source_sha256':report['source']['sha256']}

def consume(name, path):
    current = artifact(path, name)
    old = report['consumed'].get(name)
    if old and (old['sha256'], old['resolved_path']) != (current['sha256'], current['resolved_path']):
        raise ValueError(f'consumed {name} changed during validation: {path}')
    report['consumed'][name] = current
    return current

def validate_binary():
    binary = consume('hub_binary', args.binary)
    built, reference = receipt(args.build_receipt, 'build')
    matching = [item for item in built.get('artifacts', [])
                if item.get('role') == 'hub_binary' and item.get('sha256') == binary['sha256']]
    if not matching:
        raise ValueError(f'{args.build_receipt}: consumed hub binary does not match a built hub artifact')
    report['provenance']['build'] = dict(reference, artifact_path=matching[0]['path'], binary_sha256=binary['sha256'])

def validate_prebuilt():
    if not args.prebuilt_receipt:
        raise ValueError('BACKPLANE_PREBUILT=1 requires --prebuilt-receipt from the emit lane')
    emitted, reference = receipt(args.prebuilt_receipt, 'emit')
    expected = {item['path']:item for item in emitted.get('artifacts', [])
                if item.get('role') in ['emitted_c', 'emitted_web', 'emitted_wire']}
    required = {'build/cc/backplane-serve/all.c', 'dist/web/index.html'}
    if not required.issubset(expected) or not any(name.startswith('build/wire/') and name.endswith('.js') for name in expected):
        raise ValueError('emit receipt is missing the required server C, web entry or wire script')
    for name, item in expected.items():
        path = (ROOT/name).resolve()
        if not path.is_relative_to(ROOT) or artifact(path)['sha256'] != item['sha256']:
            raise ValueError(f'prebuilt input does not match emit receipt: {name}')
    for folder in ['dist/web', 'build/wire']:
        actual = {str(path.relative_to(ROOT)) for path in (ROOT/folder).glob('*') if path.is_file()}
        recorded = {name for name in expected if str(Path(name).parent) == folder}
        if actual != recorded:
            raise ValueError(f'prebuilt file set does not match emit receipt: {folder}')
    report['provenance']['prebuilt'] = dict(reference, artifacts=list(expected.values()))

def validate_cad():
    selected = os.environ.get('BACKPLANE_KICAD_CLI') or 'kicad-cli'
    cli = shutil.which(selected)
    if not cli:
        raise ValueError('real-cad requires KiCad 10 or newer; set BACKPLANE_KICAD_CLI to a compatible executable')
    binary = consume('kicad_cli', cli)
    version = capture([binary['resolved_path'], '--version'])
    report['provenance']['cad'] = dict(version, minimum_major=10)
    parsed = re.match(r'^\s*(\d+)\.', version['output'])
    if version['status'] != 0 or not parsed or int(parsed.group(1)) < 10:
        raise ValueError('real-cad requires KiCad 10 or newer for the pinned fixture; '
                         f"selected CLI reported {version['output']!r} (status {version['status']})")
    # Ensure the hub uses the exact compatible executable whose identity was
    # checked, instead of another managed installation found before PATH.
    os.environ['BACKPLANE_KICAD_CLI'] = binary['resolved_path']

def prepare_inputs():
    report['provenance']['bend_version'] = bend_version(report['toolchain'])
    if args.lane in ['native', 'browser', 'real-cad']:
        validate_binary()
    if args.lane == 'native':
        consume('phone_bridge', 'mobile/build/assets/bridge.js')
    if args.lane == 'real-cad':
        validate_cad()
    if args.lane == 'build' and os.environ.get('BACKPLANE_PREBUILT') == '1':
        validate_prebuilt()

def save():
    temp=report_path.with_suffix('.pending')
    temp.write_text(json.dumps(report,indent=2)+'\n')
    temp.replace(report_path)
save()
failed=False
try:
    prepare_inputs()
except (OSError, ValueError, KeyError, TypeError) as error:
    failed=True
    report['provenance']['error']=str(error)
    print(f'FAIL provenance: {error}', flush=True)
save()
for index, command in enumerate([] if failed else lanes[args.lane]):
    start=time.monotonic()
    log=args.out/f'{index:02d}.log'
    print('+ '+ ' '.join(command), flush=True)
    status='fail'; code=None; reason=''
    try:
        if args.binary in command and args.lane in ['native', 'browser', 'real-cad']:
            consume('hub_binary', args.binary)
        if 'mobile/build/assets/bridge.js' in command:
            consume('phone_bridge', 'mobile/build/assets/bridge.js')
        with log.open('wb') as stream:
            r=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=3600)
        code=r.returncode
        text=log.read_text(errors='replace')
        if code == 0 and not re.search(r'^(?:FAIL\b|skip(?:ped)?\b)',text,re.M|re.I):
            status='pass'
        elif re.search(r'^skip(?:ped)?\b',text,re.M|re.I):
            status='skip';reason='required lane reported skipped coverage'
    except (OSError, ValueError, subprocess.TimeoutExpired) as e:
        reason=str(e)
    if status != 'pass':
        failed=True
    report['steps'].append({'command':command,'result':status,'exit_code':code,'seconds':time.monotonic()-start,'reason':reason,'log':log.name,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest() if log.exists() else None})
    save()
    if failed:
        break
report['source_after']=source()
if report['source_after'] != report['source']:
    failed=True;report['source_changed_during_run']=True
try:
    for name, item in list(report['consumed'].items()):
        consume(name, item['resolved_path'])
    patterns = ['dist/backplane*','dist/web/*','build/release-child','build/kicad_platform','build/worktree_lifecycle_test','mobile/build/assets/bridge.js','mobile/android/app/build/outputs/apk/debug/*.apk']
    if args.lane == 'emit':
        patterns += ['build/cc/backplane-serve/all.c', 'build/wire/*']
    for pattern in patterns:
        for path in sorted(ROOT.glob(pattern)):
            if path.is_file():
                relative = str(path.relative_to(ROOT))
                role = None
                if args.lane == 'build' and relative in ['dist/backplane', 'dist/backplane-serve']:
                    role = 'hub_binary'
                elif args.lane == 'emit':
                    if relative == 'build/cc/backplane-serve/all.c': role = 'emitted_c'
                    elif relative.startswith('dist/web/'): role = 'emitted_web'
                    elif relative.startswith('build/wire/'): role = 'emitted_wire'
                report['artifacts'].append(artifact(path, role))
    if args.lane == 'build' and not any(item.get('role') == 'hub_binary' and item['path'] == 'dist/backplane' for item in report['artifacts']):
        raise ValueError('build produced no dist/backplane hub binary')
    if args.lane == 'emit' and (not {'build/cc/backplane-serve/all.c', 'dist/web/index.html'}.issubset(item['path'] for item in report['artifacts']) or not any(item.get('role') == 'emitted_wire' and item['path'].endswith('.js') for item in report['artifacts'])):
        raise ValueError('emit produced no complete server C, web entry and wire script')
    known = {item['resolved_path'] for item in report['artifacts']}
    report['artifacts'] += [item for item in report['consumed'].values() if item['resolved_path'] not in known]
except (OSError, ValueError) as error:
    failed=True;report['provenance']['error']=str(error)
report['result']='fail' if failed else 'pass'
report['finished']=datetime.now(timezone.utc).isoformat()
save()
print(f"{report['result']}: {report_path}")
sys.exit(1 if failed else 0)
