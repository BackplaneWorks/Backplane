#!/usr/bin/env python3
"""Measure pinned fast tools on repository inputs; no app execution or hook install.

Install tools/ci first. --prepare installs only the pinned comparison tools in
build/. All format writes and deliberate errors are isolated fixtures.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT/'tools/ci'
RULES = ['no-constant-binary-expression', 'no-debugger', 'no-dupe-args', 'no-dupe-keys',
         'no-duplicate-case', 'no-self-assign', 'no-sparse-arrays', 'use-isnan']


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while b := f.read(1024*1024):
            h.update(b)
    return h.hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--prepare', action='store_true')
    p.add_argument('--samples', type=int, default=5)
    p.add_argument('--out', type=Path, default=Path('build/ci-speed/fast-tools'))
    a = p.parse_args()
    if not 1 <= a.samples <= 7:
        p.error('samples must be1..7')
    os.chdir(ROOT)
    out = a.out.resolve();out.mkdir(parents=True, exist_ok=True)
    (out/'benchmark.json').unlink(missing_ok=True)
    work = ROOT/'build/ci-speed/tool-comparison';work.mkdir(parents=True, exist_ok=True)
    pins = {'eslint': '10.14.0', 'typescript6': 'npm:typescript@6.0.2'}
    (work/'package.json').write_text(json.dumps({'private':True,'type':'module','devDependencies':pins})+'\n')
    if a.prepare:
        reference = out/'comparison-bun.lock'
        if reference.exists():
            (work/'bun.lock').write_bytes(reference.read_bytes())
        install = ['bun','install','--ignore-scripts','--cwd',str(work)]
        if (work/'bun.lock').exists():
            install.append('--frozen-lockfile')
        subprocess.run(install, check=True,
                       env={**os.environ,'DO_NOT_TRACK':'1','BUN_CONFIG_NO_ANALYTICS':'1'})
    bins = TOOLS/'node_modules/.bin'
    tsc6 = work/'node_modules/typescript6/bin/tsc'
    eslint = work/'node_modules/eslint/bin/eslint.js'
    if not all(x.is_file() for x in [bins/'tsc',bins/'oxlint',bins/'oxfmt',tsc6,eslint]):
        raise ValueError('tools unavailable: install tools/ci and run benchmark --prepare explicitly')
    files = sorted(str(p.relative_to(ROOT)) for folder in ['src','desktop']
                   for p in (ROOT/folder).rglob('*') if p.is_file() and p.suffix in ['.js','.mjs','.cjs'])
    (work/'eslint.config.mjs').write_text('export default '+json.dumps([{
        'files':['**/*.{js,mjs,cjs}'],'languageOptions':{'ecmaVersion':'latest'},
        'rules':{r:'error' for r in RULES}}])+';\n')
    (work/'oxlint.json').write_text(json.dumps({'categories':{'correctness':'off'},
        'jsPlugins':[{'name':'backplane','specifier':str(TOOLS/'duplicate-parameters.mjs')}],
        'rules':{('backplane/'+r if r=='no-dupe-args' else r):'error' for r in RULES}},indent=2)+'\n')
    sources = sorted((ROOT/'tools/step2glb').glob('*.ts'))
    config = {'extends':str(ROOT/'tools/step2glb/tsconfig.json'),
              'compilerOptions':{'typeRoots':[str(TOOLS/'node_modules/@types')]},
              'files':list(map(str,sources)), 'include':[]}
    (work/'tsconfig.json').write_text(json.dumps(config,indent=2)+'\n')
    commands = {
        'eslint':['node',str(eslint),'--no-config-lookup','--no-ignore','--config',str(work/'eslint.config.mjs'),'--format','json',*files],
        'oxlint':[str(bins/'oxlint'),'--config',str(work/'oxlint.json'),'--no-ignore','--deny-warnings','--format','json','--threads','4',*files],
        'typescript6':['node',str(tsc6),'--project',str(work/'tsconfig.json'),'--pretty','false'],
        'typescript7':[str(bins/'tsc'),'--project',str(work/'tsconfig.json'),'--pretty','false'],
    }

    def call(name, command):
        record = work/(name+'.rss')
        measured = ['/usr/bin/time','-f','%M','-o',str(record),*command] if sys.platform=='linux' else command
        start=time.perf_counter();r=subprocess.run(measured,text=True,capture_output=True,timeout=30)
        return r, {'seconds':time.perf_counter()-start,
                   'peak_rss_kib':int(record.read_text().splitlines()[-1]) if sys.platform=='linux' else None}

    rows={}
    for name,command in commands.items():
        samples=[]
        for i in range(a.samples):
            r,metric=call(name,command)
            if r.returncode:
                (out/(name+'-failure.txt')).write_text(r.stdout+r.stderr)
                raise ValueError(name+' did not pass; no passing benchmark receipt produced')
            if name=='eslint':
                data=json.loads(r.stdout)
                assert len(data)==len(files) and not any(x['messages'] for x in data)
            elif name=='oxlint':
                data=json.loads(r.stdout)
                assert data['number_of_files']==len(files) and data['number_of_rules']==8 and not data['diagnostics']
            samples.append(metric)
        rows[name]={'samples':samples,'median_seconds':statistics.median(x['seconds'] for x in samples),
                    'max_peak_rss_kib':max(x['peak_rss_kib'] for x in samples) if sys.platform=='linux' else None,'exit':0}
        print(name,rows[name]['median_seconds'],flush=True)

    # Exercise the real production toolchain, not just its configuration.
    check=subprocess.run(['node',str(TOOLS/'test.mjs')],text=True,capture_output=True,timeout=10)
    if check.returncode:
        raise ValueError('negative tool controls failed: '+check.stdout+check.stderr)
    (out/'negative-controls.txt').write_text(check.stdout)
    strict,strict_metric=call('strict-oxlint',[str(bins/'oxlint'),'--config',str(TOOLS/'oxlint.json'),
        '--deny','correctness','--deny','suspicious','--deny-warnings','--format','json','--threads','4',*files])
    diagnostics=json.loads(strict.stdout)['diagnostics']
    by_rule={}
    for d in diagnostics:
        code=d.get('code','parser');by_rule[code]=by_rule.get(code,0)+1
    # A strict failure is retained as debt, never relabelled as a green gate.
    fmt,fmt_metric=call('format',[str(bins/'oxfmt'),'--check',*files])
    (out/'format-check.txt').write_text(fmt.stdout+fmt.stderr)
    source_files={Path(f) for f in files}|set(sources)|{ROOT/'tools/step2glb/tsconfig.json',ROOT/'tools/platform/windows-path.ts',
        TOOLS/'package.json',TOOLS/'bun.lock',TOOLS/'oxlint.json',TOOLS/'duplicate-parameters.mjs',TOOLS/'test.mjs',Path(__file__).resolve()}
    receipt={'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
        'platform':platform.platform(),'node':subprocess.check_output(['node','--version'],text=True).strip(),
        'bun':subprocess.check_output(['bun','--revision'],text=True).strip(),'js_files':len(files),'matched_rules':RULES,
        'packages':json.loads((TOOLS/'package.json').read_text())['devDependencies'],'comparison_pins':pins,
        'rows':rows,'strict_lint':{**strict_metric,'exit':strict.returncode,'diagnostics':len(diagnostics),'by_rule':by_rule},
        'format_check':{**fmt_metric,'exit':fmt.returncode},
        'scope':'Matched JavaScript rules and configured step2glb project; no whole-repository type coverage or pre-push readiness',
        'rss_measurement':'largest descendant process, not aggregate concurrent memory',
        'environment':{k:os.environ[k] for k in ['NODE_OPTIONS','BUN_JSC_forceRAMSize','BUN_JSC_maxPerThreadStackUsage'] if k in os.environ},
        'source_hashes':{str(p.relative_to(ROOT)) if p.is_absolute() else str(p):sha(p) for p in sorted(source_files)}}
    identities = {str(Path(shutil.which(name)).resolve()): sha(Path(shutil.which(name)).resolve()) for name in ['node','bun']}
    for pattern in ['tools/ci/node_modules/@oxlint/*/*.node', 'tools/ci/node_modules/@oxfmt/*/*.node',
                    'tools/ci/node_modules/@typescript/*/lib/tsc']:
        for binary in ROOT.glob(pattern):
            identities[str(binary.relative_to(ROOT))] = sha(binary)
    for binary in [work/'node_modules/typescript6/lib/_tsc.js', work/'node_modules/eslint/lib/linter/linter.js']:
        identities[str(binary.relative_to(ROOT))] = sha(binary)
    receipt['actual_runtime_and_implementation_hashes'] = identities
    (out/'benchmark.json').write_text(json.dumps(receipt,indent=2)+'\n')
    (out/'comparison-package.json').write_bytes((work/'package.json').read_bytes())
    (out/'comparison-bun.lock').write_bytes((work/'bun.lock').read_bytes())
    for name in ['eslint.config.mjs','oxlint.json','tsconfig.json']:
        (out/name).write_bytes((work/name).read_bytes())
    print('strict lint exit',strict.returncode,'diagnostics',len(diagnostics),'format exit',fmt.returncode,flush=True)
    return 0


if __name__=='__main__':
    try:
        sys.exit(main())
    except (AssertionError,ValueError,OSError,subprocess.SubprocessError) as error:
        print('ci-tool-bench: '+str(error),file=sys.stderr)
        sys.exit(3)
