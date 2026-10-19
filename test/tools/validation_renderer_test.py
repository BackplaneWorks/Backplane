#!/usr/bin/env python3
"""Required renderer receipts with tiny fake tools; no compiler, Bend or X server."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

RUNNER = Path('scripts/validate-contribution.py').read_text()
PLAN_PARSER = Path('test/tools/keep_plan_report.py').read_text()
GLYPH = ['native font loaded', 'native cold stream has wants',
         'native nonASCII fill drains wants', 'native layout emits keys directly',
         'native table resolves original masks', 'native keyed pixels equal original RGlyph bytes',
         'native cold to warm frame is fresh']
I18N = ['Japanese literal', 'Japanese save', 'English plural', 'Japanese count',
        'exact plural', 'select', 'interpolation', 'literal fallback', 'plural fallback']
KEEP = [f'{x},{y},{n} {case}{flag}' for x,y,n in [(0,0,128),(0,0,64),(64,0,64),(16,16,16),(256,256,32)]
        for case in ['equal','removed','inserted','reordered'] for flag in ['', ' false flag']]
KEEP += ['unchanged exact pixels', 'changed run exact pixels', 'reordered ops exact pixels',
         'changed base exact pixels', 'scroll hint still exact pixels', 'changed polygon exact pixels',
         'changed polygon negative control', 'changed run negative control', 'whole plan exact image']
COUNTS = dict.fromkeys(['masks', 'keys', 'prekeyed', 'table-guard', 'convert-and-eq', 'key-eq', 'extension-plan'], 100)
COUNTS.update(dict.fromkeys(['font-open', 'ascii-fill-table', 'nonASCII-fill-table', 'fill-redraw-and-byte-check', 'cold-frame'], 1))

with tempfile.TemporaryDirectory(prefix='bp-validation-renderer-') as tmp:
    root = Path(tmp)/'repo'
    def write(name, text, executable=False):
        path = root/name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        if executable:
            path.chmod(0o755)
        return path
    def python(name, text):
        return write(name, '#!/usr/bin/env python3\n'+text, True)
    write('scripts/validate-contribution.py', RUNNER)
    write('.gitignore', 'build/\n')
    for name in ['native_display.ts', 'native_display_test.ts', 'native_display_compare.ts',
                 'native_display_compare_test.ts', 'native_display_readback_test.ts',
                 'xpoke.c', 'present_observer_fixture.c']:
        write('test/tools/'+name, '// synthetic harness input\n')
    for name in ['glyph_table_bench', 'keep_plan_bench', 'i18n_smoke']:
        write('test/native/'+name+'.bend', '# synthetic source, never compiled\n')
    write('test/keep_step_test.bend', '# synthetic plan assertions, never compiled\n')
    write('test/tools/keep_plan_report.py', PLAN_PARSER)
    write('font.dat', 'synthetic Japanese font\n')
    fake_tool = '''import os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
if name == 'bend':
    assert sys.argv[1:] == ['version']
    print('2.0.36')
elif name == 'clang':
    if sys.argv[1:] == ['--version']:
        print('fixture clang 19')
    else:
        assert '-lX11' in sys.argv
        out = pathlib.Path(sys.argv[sys.argv.index('-o')+1])
        if not os.getenv('CONTROL_NO_XPOKE'):
            out.write_text('#!/bin/sh\\nexit 0\\n'); out.chmod(0o755)
elif name == 'fc-match':
    print(pathlib.Path('font.dat').resolve())
    print('en' if os.getenv('CONTROL_NO_JAPANESE') else 'en|ja')
elif name == 'bun':
    test = sys.argv[1]
    if test == '--version': print('fixture Bun 1')
    elif test.endswith('/native_display_test.ts'):
        print('native display demand: changed target accepted; stale, wrong, indistinguishable and late content rejected')
        print('composer observer: caret blink ignored; missing/corrupt glyph pixels and wrong region size distinguished')
        if not os.getenv('CONTROL_OLD_POLICY'):
            print('calibration: delayed/transient frames settle; unchanged, wrong-empty, oscillating and late pixels rejected (7 controls)')
        if os.getenv('CONTROL_SOURCE_CHANGE'): pathlib.Path('font.dat').write_text('changed source')
    elif test.endswith('/native_display_compare_test.ts'):
        print(('28' if os.getenv('CONTROL_OLD_COMPARE') else '29')+' independent native display comparison controls passed')
    elif test.endswith('/native_display_readback_test.ts'):
        assert len(sys.argv) == 5
        assert all(pathlib.Path(p).is_file() for p in sys.argv[2:])
        assert os.environ['HOME'] != os.environ['CONTROL_ORIGINAL_HOME']
        if not os.getenv('CONTROL_NO_READBACK'):
            print('xpoke pixels independently read exact red/green RGB bytes from X-server fixture')
    else: raise AssertionError(test)
elif name == 'Xvfb': raise AssertionError('fake Bun must not start an X server')
else: raise AssertionError(name)
'''
    for name in ['bun', 'bend', 'clang', 'Xvfb', 'fc-match']:
        python('bin/'+name, fake_tool)
    runtime = '''import os, pathlib, sys
assert os.environ['HOME'] != os.environ['CONTROL_ORIGINAL_HOME']
assert os.environ['XDG_CACHE_HOME'].startswith(os.environ['HOME']+'/')
assert os.environ['BACKPLANE_HOME'].startswith(os.environ['HOME']+'/')
glyph = pathlib.Path(sys.argv[0]).name == 'glyph_table_bench'
plan = pathlib.Path(sys.argv[0]).name == 'keep_plan_bench'
checks = KEEP if plan else GLYPH if glyph else ['native '+name for name in I18N]
for check in checks:
    if plan and os.getenv('CONTROL_MISSING_PLAN_ASSERT') and check == 'whole plan exact image':
        print('ok '+checks[0]); continue  # Same count cannot replace a missing named assertion.
    if os.getenv('CONTROL_MISSING_ASSERT') == ('glyph' if glyph else 'i18n') and check == checks[-1]: continue
    if os.getenv('CONTROL_MISSING_ASSERT') == 'rglyph' and 'RGlyph bytes' in check: continue
    print('ok '+check)
if os.getenv('CONTROL_ERROR'): print('Error: fixture failure despite zero exit')
if os.getenv('CONTROL_SKIP'): print('skip required renderer check')
if plan:
    for i in range(100):
        count = 18 if i == 0 and os.getenv('CONTROL_BAD_PLAN_COUNT') else 17
        print(f'sample,{i},{i+10},{i+2},17,{count}')
fixture = 'keep_plan_bench' if os.getenv('CONTROL_KEEP_PLAN') else 'glyph_table_bench'
if not glyph and not plan and os.getenv('CONTROL_MUTATE_BINARY'):
    with (pathlib.Path(sys.argv[0]).parent/fixture).open('a') as f: f.write('# changed after consumption\\n')
if not glyph and not plan and os.getenv('CONTROL_MUTATE_C'):
    with pathlib.Path('build/cc',fixture,'all.c').open('a') as f: f.write('changed after compilation\\n')
'''
    python('scripts/build-app.sh', '''import os, pathlib, sys
assert os.environ['HOME'] == os.environ['CONTROL_ORIGINAL_HOME'], 'compiler lost its toolchain HOME'
assert os.environ['BACKPLANE_PREBUILT'] == '0'
assert os.environ['BACKPLANE_EMIT_ONLY'] == '0'
assert os.environ['BACKPLANE_GOOGLE_BAKE'] == '0'
out = pathlib.Path(sys.argv[2]); work = pathlib.Path('build/cc')/out.name
work.mkdir(parents=True, exist_ok=True)
if not os.getenv('CONTROL_NO_C'): (work/'all.c').write_text('fresh synthetic C\\n')
out.write_text(RUNTIME); out.chmod(0o755)
'''.replace('RUNTIME', repr('#!/usr/bin/env python3\nGLYPH = '+repr(GLYPH)+'\nI18N = '+repr(I18N)+'\nKEEP = '+repr(KEEP)+'\n'+runtime)))
    python('test/tools/glyph_bench_report.py', '''import json, os, pathlib, sys
if not os.getenv('CONTROL_NO_SUMMARY'):
    counts = COUNTS.copy()
    if os.getenv('CONTROL_INCOMPLETE_SAMPLES'): counts['masks'] = 99
    pathlib.Path(sys.argv[2]).write_text(json.dumps({'samples': {name:{'n':n} for name,n in counts.items()}}))
print('705 complete native glyph samples summarized')
'''.replace('COUNTS', repr(COUNTS)))
    env = dict(os.environ, PATH=str(root/'bin')+os.pathsep+os.environ['PATH'],
               CONTROL_ORIGINAL_HOME=os.environ['HOME'], GIT_CONFIG_NOSYSTEM='1',
               GIT_CONFIG_GLOBAL=str(Path(tmp)/'no-config'))
    for command in [['git', 'init', '-q'], ['git', 'add', '.'],
                    ['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture']]:
        subprocess.run(command, cwd=root, env=env, check=True, stdout=subprocess.DEVNULL)
    def run(profile=None, **settings):
        command = ['python3', 'scripts/validate-contribution.py', 'renderer']
        if profile is not None:
            command += ['--renderer-profile', profile]
            settings['CONTROL_KEEP_PLAN'] = '1' if profile == 'keep-plan' else ''
        code = subprocess.run(command, cwd=root, env=dict(env, **settings), stdout=subprocess.DEVNULL).returncode
        report = json.loads((root/'build/validation/renderer/result.json').read_text())
        assert (code == 0) == (report['result'] == 'pass')
        return code, report
    harness = root/'test/tools/present_observer_fixture.c'
    data = harness.read_bytes(); harness.unlink()
    code, r = run()
    assert code and not r['steps'] and '62fa318' in r['provenance']['error']
    harness.write_bytes(data)
    print('ok absent native integration is an explicit required dependency failure')
    code, r = run(BACKPLANE_TEST_XVFB=str(root/'missing-Xvfb'))
    assert code and not r['steps'] and 'requires executable' in r['provenance']['error']
    code, r = run(CONTROL_NO_JAPANESE='1')
    assert code and not r['steps'] and 'Japanese-capable font' in r['provenance']['error']
    print('ok missing Xvfb and missing Japanese font fail before tests')
    code, r = run(BACKPLANE_PREBUILT='1', BACKPLANE_EMIT_ONLY='1')
    assert not code and len(r['steps']) == 10 and r['source'] == r['source_after']
    assert r['renderer_profile'] == r['provenance']['renderer']['profile'] == 'glyph-cache'
    for item in r['artifacts']:
        assert item['sha256'] == hashlib.sha256(Path(item['resolved_path']).read_bytes()).hexdigest()
    assert len([a for a in r['artifacts'] if a.get('role') == 'renderer_emitted_c']) == 2
    assert len([a for a in r['consumed'] if a in ['renderer_xpoke', 'renderer_present_observer_fixture', 'renderer_glyph_table_bench', 'renderer_i18n_native_smoke']]) == 4
    assert not Path(r['steps'][6]['environment']['HOME']).exists(), 'private runtime HOME was retained'
    assert all('check.sh' not in ' '.join(s['command']) and 'test.sh' not in ' '.join(s['command']) for s in r['steps'])
    print('ok complete required work binds compiled fixtures/C/logs/source and isolates runtime HOME without repeating proof/units')
    for mode, index in [('CONTROL_OLD_POLICY', 0), ('CONTROL_OLD_COMPARE', 1), ('CONTROL_NO_READBACK', 4)]:
        code, r = run(**{mode:'1'})
        assert code and r['steps'][-1]['exit_code'] == 0 and len(r['steps']) == index+1
    print('ok old settling/comparison coverage and zero-exit missing X11 readback cannot pass')
    for kind in ['glyph', 'rglyph', 'i18n']:
        code, r = run(CONTROL_MISSING_ASSERT=kind)
        assert code and r['steps'][-1]['exit_code'] == 0 and 'assertions missing' in r['steps'][-1]['reason']
    print('ok missing RGlyph/fresh-frame or native i18n assertions fail even at exit zero')
    for mode in ['CONTROL_ERROR', 'CONTROL_SKIP']:
        code, r = run(**{mode:'1'})
        assert code and r['steps'][-1]['result'] != 'pass'
    print('ok Error and skipped required renderer work fail visibly')
    for mode in ['CONTROL_NO_XPOKE', 'CONTROL_NO_C']:
        # Prior successful outputs exist: they must be removed before compiling.
        code, r = run(**{mode:'1'})
        assert code and r['steps'][-1]['exit_code'] == 0 and r['steps'][-1]['reason']
    print('ok missing fresh binary or emitted C cannot reuse stale successful outputs')
    for mode in ['CONTROL_INCOMPLETE_SAMPLES', 'CONTROL_NO_SUMMARY']:
        code, r = run(**{mode:'1'})
        assert code and r['provenance']['error']
    print('ok incomplete 705-sample summary and absent replacement summary fail')
    for mode, name in [('CONTROL_MUTATE_BINARY', 'renderer_glyph_table_bench'), ('CONTROL_MUTATE_C', 'renderer_glyph_table_bench_emitted_c')]:
        code, r = run(**{mode:'1'})
        assert code and 'consumed '+name+' changed' in r['provenance']['error']
    print('ok compiled fixture or emitted C replacement after execution invalidates evidence')
    code, r = run(CONTROL_SOURCE_CHANGE='1')
    assert code and r['source_changed_during_run']
    print('ok changing renderer source during the lane invalidates otherwise passing steps')
    glyph = root/'test/native/glyph_table_bench.bend'; glyph_data=glyph.read_bytes(); glyph.unlink()
    code,r=run()
    assert code and 'glyph_table_bench.bend' in r['provenance']['error'] and not r['steps']
    code,r=run(profile='keep-plan')
    assert not code and r['renderer_profile'] == r['provenance']['renderer']['profile'] == 'keep-plan'
    assert any(a.get('role') == 'renderer_keep_plan_summary' for a in r['artifacts'])
    assert not any('glyph_table_bench' in ' '.join(s['command']) for s in r['steps'])
    assert not any('glyph-summary' in a['path'] for a in r['artifacts'])
    glyph.write_bytes(glyph_data)
    print('ok glyph-cache remains the required default; explicit keep-plan passes 49 controls/100 real-parser samples without claiming glyph 705')
    plan=root/'test/native/keep_plan_bench.bend';plan_data=plan.read_bytes();plan.unlink()
    code,r=run(profile='keep-plan')
    assert code and not r['steps'] and 'main-only Keep.step' in r['provenance']['error']
    plan.write_bytes(plan_data)
    print('ok an explicitly selected missing keep-plan fixture cannot fall back to available glyph coverage')
    code,r=run(profile='keep-plan',CONTROL_MISSING_PLAN_ASSERT='1')
    assert code and r['steps'][-1]['exit_code']==0 and 'whole plan exact image' in r['steps'][-1]['reason']
    code,r=run(profile='keep-plan',CONTROL_BAD_PLAN_COUNT='1')
    assert code and r['steps'][-1]['exit_code']!=0
    print('ok 49-count output missing a named control and unequal reference/current plan counts both fail')
    code,r=run(profile='keep-plan',CONTROL_MUTATE_BINARY='1')
    assert code and 'consumed renderer_keep_plan_bench changed' in r['provenance']['error']
    print('ok explicitly profiled keep-plan binary mutation invalidates the receipt')
