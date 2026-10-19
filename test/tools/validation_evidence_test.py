#!/usr/bin/env python3
"""Validation must distinguish pass, skip, missing commands and source changes."""
import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

runner=Path('scripts/validate-contribution.py').read_text()
with tempfile.TemporaryDirectory(prefix='bp-validation-evidence-') as d:
    root=Path(d)/'repo';(root/'scripts').mkdir(parents=True);(root/'scripts/validate-contribution.py').write_text(runner)
    (root/'.gitignore').write_text('build/\ndist/\nmobile/build/\n')
    (root/'source').write_text('initial\n')
    def script(path, text):
        path=root/path;path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text('#!/bin/sh\nset -eu\n'+text);path.chmod(0o755)
    for name in ['check','test']:
        script(f'scripts/{name}.sh','echo "ok control"\n')
    # Every compiler/browser command is a tiny stand-in; this suite never
    # launches Bend, native compilation, browsers or a real hub.
    script('scripts/build.sh', '''
if [ "${1:-}" = emit ]; then
  mkdir -p build/cc/backplane-serve build/wire dist/web
  printf 'emitted C from fixture\\n' > build/cc/backplane-serve/all.c
  printf 'fixture wire\\n' > build/wire/chunk.js
  printf 'fixture web\\n' > dist/web/index.html
else
  mkdir -p dist
  printf '#!/bin/sh\\necho fixture hub\\n' > dist/backplane
  chmod +x dist/backplane
  cp dist/backplane dist/backplane-serve
fi
echo 'ok fixture build'
''')
    script('scripts/build-wire.sh', 'mkdir -p build/wire\nprintf "fixture wire\\n" > build/wire/chunk.js\n')
    script('scripts/build-app.sh', '''
mkdir -p "$(dirname "$2")"
printf '#!/bin/sh\\necho ok fixture native\\n' > "$2"
chmod +x "$2"
''')
    (root/'scripts/fetch-cad-fixture.py').write_text("print('ok fixture CAD')\n")
    script('bin/kicad-cli', 'printf "%s\\n" "${EVIDENCE_KICAD_VERSION:-10.0.6}"\nexit "${EVIDENCE_KICAD_STATUS:-0}"\n')
    for name in ['bun','bend','clang']:
        script(f'bin/{name}', '''
if [ "${EVIDENCE_MUTATE_BRIDGE:-}" = 1 ] && [ "${1:-}" = test/tools/outbox_e2e.ts ]; then
  printf 'changed\\n' >> mobile/build/assets/bridge.js
fi
if [ "${EVIDENCE_MUTATE_BINARY:-}" = 1 ] && [ "${1:-}" = test/tools/teach_e2e.ts ]; then
  printf 'changed\\n' >> "$2"
fi
echo 'ok fixture tool'
''')
    env=dict(os.environ, PATH=str(root/'bin')+os.pathsep+os.environ['PATH'],
             GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=str(Path(d)/'no-config'))
    for command in [['git','init','-q'],['git','add','.'],['git','-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-qm','fixture']]:
        subprocess.run(command,cwd=root,env=env,check=True,stdout=subprocess.DEVNULL)
    def run(lane='pure', *options, extra_env=None):
        result=subprocess.run(['python3','scripts/validate-contribution.py',lane,*map(str,options)],cwd=root,env=dict(env,**(extra_env or {})),stdout=subprocess.DEVNULL)
        output=Path(options[options.index('--out')+1]) if '--out' in options else Path('build/validation')
        evidence=json.loads((root/output/lane/'result.json').read_text())
        return result.returncode,evidence
    code,e=run();assert code==0 and e['result']=='pass' and all(s['result']=='pass' for s in e['steps'])
    print('ok complete required commands pass with command/source evidence')
    (root/'scripts/check.sh').write_text('#!/bin/sh\necho "skip required proof"\n')
    code,e=run();assert code and e['result']=='fail' and e['steps'][0]['result']=='skip'
    print('ok skipped required work is visible and fails acceptance')
    (root/'scripts/check.sh').unlink()
    code,e=run();assert code and e['result']=='fail' and e['steps'][0]['reason']
    print('ok missing required executable fails with a concrete reason')
    p=root/'scripts/check.sh';p.write_text('#!/bin/sh\necho changed >> source\necho "ok control"\n');p.chmod(0o755)
    code,e=run();assert code and e['source_changed_during_run'] and e['source_after']['sha256']!=e['source']['sha256']
    print('ok source mutation during otherwise passing tests invalidates evidence')
    script('scripts/check.sh','echo "ok control"\n')
    bridge=root/'mobile/build/assets/bridge.js';bridge.parent.mkdir(parents=True);bridge.write_text('fixture bridge\n')
    for lane in ['native','browser','real-cad']:
        (root/'dist').mkdir(exist_ok=True);(root/'dist/backplane').write_text('stale hub\n')
        code,e=run(lane)
        assert code and e['result']=='fail' and not e['steps'] and e['provenance']['error']
    print('ok every hub-consuming lane refuses binaries without a passing build receipt')
    original_source=(root/'source').read_bytes()
    code,built=run('build');assert code==0 and built['result']=='pass'
    build_receipt=root/'build/validation/build/result.json'
    saved_receipt=build_receipt.read_bytes()
    expected=hashlib.sha256((root/'dist/backplane').read_bytes()).hexdigest()
    assert any(a.get('role')=='hub_binary' and a['sha256']==expected for a in built['artifacts'])
    for lane in ['native','browser','real-cad']:
        code,e=run(lane);assert code==0 and e['result']=='pass' and e['provenance']['build']['binary_sha256']==expected
    assert e['consumed']['hub_binary']['resolved_path']==str(root/'dist/backplane')
    print('ok matching build receipt enables all hub lanes with the actual binary identity')
    assert e['consumed']['kicad_cli']['sha256']==hashlib.sha256((root/'bin/kicad-cli').read_bytes()).hexdigest()
    for version in ['8.0.9','unrecognized version']:
        code,e=run('real-cad',extra_env={'EVIDENCE_KICAD_VERSION':version})
        assert code and not e['steps'] and 'requires KiCad 10 or newer' in e['provenance']['error']
    code,e=run('real-cad',extra_env={'EVIDENCE_KICAD_STATUS':'23'})
    assert code and not e['steps'] and e['provenance']['cad']['status']==23
    print('ok old, unknown and failed CAD version probes cannot count as real-CAD acceptance')
    external=Path(d)/'copied-hub';external.write_bytes((root/'dist/backplane').read_bytes());external.chmod(0o755)
    code,e=run('browser','--binary',external,'--build-receipt',build_receipt)
    assert code==0 and e['consumed']['hub_binary']['path']==str(external)
    assert any(a['resolved_path']==str(external) and a['sha256']==expected for a in e['artifacts'])
    print('ok an external copied binary is hashed and tied to its named build receipt')
    external.write_text('unrelated stale hub\n')
    code,e=run('browser','--binary',external)
    assert code and not e['steps'] and 'does not match a built hub artifact' in e['provenance']['error']
    assert e['consumed']['hub_binary']['sha256']==hashlib.sha256(external.read_bytes()).hexdigest()
    assert (root/'dist/backplane').read_bytes()!=external.read_bytes()
    print('ok an unrelated override fails even while the default dist binary still matches')
    (root/'source').write_text('changed source\n')
    code,e=run('browser');assert code and not e['steps'] and 'receipt source does not match' in e['provenance']['error']
    (root/'source').write_bytes(original_source)
    print('ok a binary from older source cannot receive current-source acceptance')
    invalid=json.loads(saved_receipt);invalid['result']='fail';build_receipt.write_text(json.dumps(invalid))
    code,e=run('browser');assert code and not e['steps']
    build_receipt.write_text('[]')
    code,e=run('browser');assert code and not e['steps']
    build_receipt.write_bytes(saved_receipt)
    print('ok failed and malformed producer receipts fail before any consumer test')
    code,e=run('native')
    assert code==0 and e['consumed']['phone_bridge']['sha256']==hashlib.sha256(bridge.read_bytes()).hexdigest()
    code,e=run('native',extra_env={'EVIDENCE_MUTATE_BRIDGE':'1'})
    assert code and 'consumed phone_bridge changed' in e['provenance']['error']
    bridge.write_text('fixture bridge\n')
    print('ok the exact reconnect bridge is recorded and mutation invalidates acceptance')
    external.write_bytes((root/'dist/backplane').read_bytes())
    code,e=run('browser','--binary',external,extra_env={'EVIDENCE_MUTATE_BINARY':'1'})
    assert code and 'consumed hub_binary changed' in e['provenance']['error']
    print('ok replacing a tested external binary during the lane invalidates acceptance')
    code,emitted=run('emit');assert code==0 and emitted['result']=='pass'
    emit_receipt=root/'build/validation/emit/result.json'
    code,e=run('build',extra_env={'BACKPLANE_PREBUILT':'1'})
    assert code and not e['steps'] and 'requires --prebuilt-receipt' in e['provenance']['error']
    code,e=run('build','--prebuilt-receipt',emit_receipt,extra_env={'BACKPLANE_PREBUILT':'1'})
    assert code==0 and e['provenance']['prebuilt']['receipt_sha256']==hashlib.sha256(emit_receipt.read_bytes()).hexdigest()
    assert any(a.get('role')=='hub_binary' for a in e['artifacts'])
    code,e=run('browser');assert code==0
    print('ok matched cross-platform emit inputs produce a fresh usable build receipt')
    emitted_c=root/'build/cc/backplane-serve/all.c';original_c=emitted_c.read_bytes();emitted_c.write_text('stale C\n')
    code,e=run('build','--prebuilt-receipt',emit_receipt,extra_env={'BACKPLANE_PREBUILT':'1'})
    assert code and not e['steps'] and 'prebuilt input does not match' in e['provenance']['error']
    emitted_c.write_bytes(original_c)
    extra=root/'dist/web/unrecorded.js';extra.write_text('stale web chunk\n')
    code,e=run('build','--prebuilt-receipt',emit_receipt,extra_env={'BACKPLANE_PREBUILT':'1'})
    assert code and not e['steps'] and 'prebuilt file set does not match' in e['provenance']['error']
    extra.unlink()
    print('ok stale emitted C and unexpected web files cannot claim a matched Mac build')
    code,e=run('build','--out','evidence-output');assert code==0 and e['source']==e['source_after']
    code,e=run('browser','--out','evidence-output')
    assert code==0 and e['provenance']['build']['receipt']==str(root/'evidence-output/build/result.json')
    print('ok custom evidence output stays outside source identity across producer and consumer lanes')
