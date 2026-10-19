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
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
os.chdir(ROOT)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('lane', choices=['pure', 'native', 'renderer', 'browser', 'phone', 'real-cad', 'build', 'emit'])
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

def capture(command, env=None):
    try:
        r = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20, env=env)
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
    ['bun','test/tools/ios_lifetime_state.ts','mobile/build/assets/bridge.js',str(args.out/'ios-lifetime-state.json')],
 ],
 'real-cad': [
    ['python3','scripts/fetch-cad-fixture.py'],
    ['scripts/build-wire.sh'],
    ['bun','test/tools/review_e2e.ts',args.binary,'build/wire'],
    ['bun','test/tools/hist_e2e.ts',args.binary,'build/wire'],
 ],
 'build': [['scripts/build.sh']],
 'emit': [['scripts/build.sh', 'emit']],
 'renderer': [],  # Filled after checking the required Linux harness inputs.
}
renderer_steps = []
renderer_home = None
renderer_outputs = {}
report = {'schema':2,'lane':args.lane,'started':datetime.now(timezone.utc).isoformat(),
          'source':source(),'host':{'platform':platform.platform(),'machine':platform.machine()},
          'toolchain':[capture(c) for c in [['bend','version'],['bun','--version'],['clang','--version'],['git','--version'],['python3','--version']]],
          'flags':{k:os.environ.get(k) for k in ['CC','BACKPLANE_CFLAGS','BACKPLANE_LOW_MEM','BACKPLANE_JOBS','BACKPLANE_CPUS','BACKPLANE_PREBUILT','BACKPLANE_GOOGLE_BAKE','BACKPLANE_KICAD_CLI','BUN_JSC_forceRAMSize','BUN_JSC_useJIT','BACKPLANE_CHROME','BACKPLANE_TEST_BROWSERS','BACKPLANE_X11','BACKPLANE_TEST_XVFB']},
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

def prepare_renderer():
    global renderer_home
    if platform.system() != 'Linux':
        raise ValueError('renderer requires Linux/X11; the Mac hub lane does not cover window rendering')
    required = [
        'test/tools/native_display.ts', 'test/tools/native_display_test.ts',
        'test/tools/native_display_compare.ts', 'test/tools/native_display_compare_test.ts',
        'test/tools/native_display_readback_test.ts', 'test/tools/xpoke.c',
        'test/tools/present_observer_fixture.c', 'test/tools/glyph_bench_report.py',
        'test/native/glyph_table_bench.bend', 'test/native/i18n_smoke.bend',
        'scripts/build-app.sh',
    ]
    dependency = 'native-glyph 62fa318989b092aa8712825a4c75f1e4f0c6bcbf (or its integrated successor)'
    report['provenance']['renderer'] = {'required_dependency': dependency, 'inputs': []}
    missing = [name for name in required if not (ROOT/name).is_file()]
    if missing:
        raise ValueError(f'renderer requires {dependency}; missing: {", ".join(missing)}')
    report['provenance']['renderer']['inputs'] = [artifact(name, 'renderer_source') for name in required]
    tools = {'renderer_cc': os.environ.get('CC') or 'clang',
             'renderer_xvfb': os.environ.get('BACKPLANE_TEST_XVFB') or 'Xvfb',
             'renderer_bun': 'bun', 'renderer_bend': 'bend', 'renderer_fontconfig': 'fc-match'}
    for name, selected in tools.items():
        executable = shutil.which(selected)
        if not executable:
            raise ValueError(f'renderer requires executable {selected}')
        consume(name, executable)
    cc = report['consumed']['renderer_cc']['resolved_path']
    compiler = capture([cc, '--version'])
    report['toolchain'].append(compiler)
    if compiler['status'] != 0 or not compiler['output']:
        raise ValueError('renderer compiler version probe failed')
    renderer_home = tempfile.TemporaryDirectory(prefix='bp-renderer-home-')
    isolated = {'HOME': renderer_home.name, 'XDG_CACHE_HOME': renderer_home.name+'/cache',
                'BACKPLANE_HOME': renderer_home.name+'/backplane'}
    font = capture([report['consumed']['renderer_fontconfig']['resolved_path'], '-f', '%{file}\n%{lang}\n', ':lang=ja'],
                   env=dict(os.environ, **isolated))
    report['provenance']['renderer']['japanese_font'] = font
    lines = font['output'].splitlines()
    if font['status'] != 0 or len(lines) != 2 or not re.search(r'(?:^|\|)ja(?:\||$)', lines[1]):
        raise ValueError('renderer requires a Japanese-capable font; install fonts-noto-cjk and fontconfig')
    consume('renderer_japanese_font', lines[0])
    build_env = {'CC': cc, 'BACKPLANE_PREBUILT': '0', 'BACKPLANE_EMIT_ONLY': '0', 'BACKPLANE_GOOGLE_BAKE': '0'}
    x11 = os.environ.get('BACKPLANE_X11')
    includes = ['-I'+x11+'/include', '-L'+x11+'/lib/x86_64-linux-gnu'] if x11 else []
    for name in ['xpoke', 'present_observer_fixture', 'glyph_table_bench', 'i18n_native_smoke']:
        renderer_outputs[name] = str(args.out/name)
    def step(command, **details):
        renderer_steps.append(dict(command=command, **details))
    step(['bun', 'test/tools/native_display_test.ts'], required_lines=[
        'native display demand: changed target accepted; stale, wrong, indistinguishable and late content rejected',
        'composer observer: caret blink ignored; missing/corrupt glyph pixels and wrong region size distinguished',
        'calibration: delayed/transient frames settle; unchanged, wrong-empty, oscillating and late pixels rejected (7 controls)'])
    step(['bun', 'test/tools/native_display_compare_test.ts'], required_lines=['29 independent native display comparison controls passed'])
    for name, source_path in [('xpoke', 'test/tools/xpoke.c'), ('present_observer_fixture', 'test/tools/present_observer_fixture.c')]:
        step([cc, '-O2', *includes, source_path, '-o', renderer_outputs[name], '-lX11'], produces=[name])
    step(['bun', 'test/tools/native_display_readback_test.ts', renderer_outputs['xpoke'],
          renderer_outputs['present_observer_fixture'], report['consumed']['renderer_xvfb']['resolved_path']],
         env=isolated, consumes=['xpoke', 'present_observer_fixture'], required_lines=[
             'xpoke pixels independently read exact red/green RGB bytes from X-server fixture'])
    step(['scripts/build-app.sh', 'test/native/glyph_table_bench.bend', renderer_outputs['glyph_table_bench']],
         env=build_env, produces=['glyph_table_bench'], emitted_c='build/cc/glyph_table_bench/all.c',
         remove=['build/cc/glyph_table_bench/all.c'])
    step([renderer_outputs['glyph_table_bench']], env=isolated, consumes=['glyph_table_bench'], required_lines=[
        'ok '+name for name in ['native font loaded', 'native cold stream has wants',
        'native nonASCII fill drains wants', 'native layout emits keys directly',
        'native table resolves original masks', 'native keyed pixels equal original RGlyph bytes',
        'native cold to warm frame is fresh']])
    glyph_log = args.out/f'{len(renderer_steps)-1:02d}.log'
    step(['python3', 'test/tools/glyph_bench_report.py', str(glyph_log), str(args.out/'glyph-summary.json')],
         remove=[str(args.out/'glyph-summary.json')], required_lines=['705 complete native glyph samples summarized'])
    step(['scripts/build-app.sh', 'test/native/i18n_smoke.bend', renderer_outputs['i18n_native_smoke']],
         env=build_env, produces=['i18n_native_smoke'], emitted_c='build/cc/i18n_native_smoke/all.c',
         remove=['build/cc/i18n_native_smoke/all.c'])
    step([renderer_outputs['i18n_native_smoke']], env=isolated, consumes=['i18n_native_smoke'], required_lines=[
        'ok native '+name for name in ['Japanese literal', 'Japanese save', 'English plural',
        'Japanese count', 'exact plural', 'select', 'interpolation', 'literal fallback', 'plural fallback']])
    lanes['renderer'] = [item['command'] for item in renderer_steps]
    report['provenance']['renderer']['scope'] = 'Native rendering correctness and raw microbenchmarks; whole-app display budgets are separate.'

def prepare_inputs():
    report['provenance']['bend_version'] = bend_version(report['toolchain'])
    if args.lane in ['native', 'browser', 'real-cad']:
        validate_binary()
    if args.lane == 'native':
        consume('phone_bridge', 'mobile/build/assets/bridge.js')
        base = '30884966ef59c383581bcd1b7653f8f739cf009e'
        path = 'src/server/kicad.bend'
        probe = capture(['git', 'cat-file', '-e', base+':'+path])
        report['provenance']['native_baseline'] = dict(probe, commit=base, path=path)
        if probe['status'] != 0:
            raise ValueError('native KiCad negative control requires the exact baseline object; run '
                             f'git fetch --no-tags --depth=1 origin {base} before native acceptance')
    if args.lane == 'real-cad':
        validate_cad()
    if args.lane == 'build' and os.environ.get('BACKPLANE_PREBUILT') == '1':
        validate_prebuilt()
    if args.lane == 'renderer':
        prepare_renderer()

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
    details = renderer_steps[index] if args.lane == 'renderer' else {}
    start=time.monotonic()
    log=args.out/f'{index:02d}.log'
    print('+ '+ ' '.join(command), flush=True)
    status='fail'; code=None; reason=''
    try:
        for name in details.get('consumes', []):
            consume('renderer_'+name, renderer_outputs[name])
        for path in details.get('remove', []) + [renderer_outputs[name] for name in details.get('produces', [])]:
            Path(path).unlink(missing_ok=True)
        if args.binary in command and args.lane in ['native', 'browser', 'real-cad']:
            consume('hub_binary', args.binary)
        if 'mobile/build/assets/bridge.js' in command:
            consume('phone_bridge', 'mobile/build/assets/bridge.js')
        with log.open('wb') as stream:
            r=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,timeout=3600,
                             env=dict(os.environ, **details.get('env', {})))
        code=r.returncode
        text=log.read_text(errors='replace')
        if code == 0 and not re.search(r'^(?:FAIL\b|skip(?:ped)?\b)',text,re.M|re.I):
            status='pass'
        elif re.search(r'^skip(?:ped)?\b',text,re.M|re.I):
            status='skip';reason='required lane reported skipped coverage'
        if status == 'pass' and args.lane == 'renderer':
            missing = [line for line in details.get('required_lines', []) if line not in text.splitlines()]
            if missing or re.search(r'^Error:', text, re.M):
                status='fail';reason='required renderer assertions missing or failed: '+', '.join(missing)
            else:
                for name in details.get('produces', []):
                    consume('renderer_'+name, renderer_outputs[name])
                if details.get('emitted_c'):
                    name = details['produces'][0]
                    consume('renderer_'+name+'_emitted_c', details['emitted_c'])
                    copied = args.out/(name+'.c')
                    shutil.copy2(details['emitted_c'], copied)
                    consume('renderer_'+name+'_emitted_c_evidence', copied)
                    report['artifacts'].append(artifact(copied, 'renderer_emitted_c'))
    except (OSError, ValueError, subprocess.TimeoutExpired) as e:
        status='fail';reason=str(e)
    if status != 'pass':
        failed=True
    report['steps'].append({'command':command,'result':status,'exit_code':code,'seconds':time.monotonic()-start,'reason':reason,'log':log.name,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest() if log.exists() else None,
                            **({'environment':details['env']} if details.get('env') else {})})
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
    if args.lane == 'phone':
        state = artifact(args.out/'ios-lifetime-state.json', 'sanitized_phone_state')
        metadata_path = args.out/'ios-lifetime-state.json.metadata.json'
        metadata = json.loads(metadata_path.read_text())
        if not isinstance(metadata, dict) or metadata.get('bridgeSha256') != report['consumed'].get('phone_bridge', {}).get('sha256') or metadata.get('stateSha256') != state['sha256'] or metadata.get('rpcExecution') is not False:
            raise ValueError('sanitized Worker state does not match the exact bridge/state identities')
        report['artifacts'] += [state, artifact(metadata_path, 'sanitized_phone_state_metadata')]
    if args.lane == 'renderer' and not failed:
        summary_path = args.out/'glyph-summary.json'
        summary = json.loads(summary_path.read_text())
        expected = dict.fromkeys(['masks', 'keys', 'prekeyed', 'table-guard', 'convert-and-eq', 'key-eq', 'extension-plan'], 100)
        expected.update(dict.fromkeys(['font-open', 'ascii-fill-table', 'nonASCII-fill-table', 'fill-redraw-and-byte-check', 'cold-frame'], 1))
        if not isinstance(summary, dict) or not isinstance(summary.get('samples'), dict) or {name:item.get('n') for name,item in summary['samples'].items() if isinstance(item,dict)} != expected:
            raise ValueError('renderer requires the complete 705-sample native glyph summary')
        report['artifacts'].append(artifact(summary_path, 'renderer_glyph_summary'))
except (OSError, ValueError) as error:
    failed=True;report['provenance']['error']=str(error)
report['result']='fail' if failed else 'pass'
report['finished']=datetime.now(timezone.utc).isoformat()
if renderer_home:
    renderer_home.cleanup()
save()
print(f"{report['result']}: {report_path}")
sys.exit(1 if failed else 0)
