"""Prepare isolated TEST-only consumers. No compiler, build or workflow runs.

Frozen release verifier/producer sources stay untouched. Only explicit TEST
receipt dispatch is changed in external copies; structural guard expressions
remain present. Full before/after hashes and each replacement are recorded.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
COMMIT = 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4'
TREE = 'ec6c85b4a8b62a0227f09151a9ec52a362b20ae2'
TEST = 'TEST_ONLY_UNQUALIFIED_SERVICE_BUILD'
NORMAL = 'VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD'


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    source, out = a.source.resolve(), a.output.absolute()
    def git(*args):
        return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()
    assert git('rev-parse', 'HEAD') == COMMIT and git('rev-parse', 'HEAD^{tree}') == TREE
    assert not git('status', '--porcelain', '--untracked-files=no') and not out.exists()
    out.mkdir(parents=True)
    edits = []
    tracked = git('ls-files', 'scripts', 'desktop', 'deploy/kistack.lock').splitlines()
    for name in tracked:
        src = source / name
        assert not src.is_symlink() and src.is_file()
        dest = out / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
    def edit(name, old, new, count=1):
        path = out / name
        text = path.read_text()
        assert text.count(old) == count, (name, old, text.count(old))
        path.write_text(text.replace(old, new))
        edits.append({'path': name, 'old': old, 'new': new, 'count': count})
    service = 'scripts/service-release.py'
    edit(service, NORMAL, TEST, 2)
    edit(service, '    bend = check_tool(request["toolchain"]["bend"], base, "bend")',
         '    from test_service_hooks import TestHooks\n'
         '    hooks = TestHooks(sys.modules[__name__], request, base)\n'
         '    bend = hooks.compiler()')
    edit(service, 'qualification.get("status") == "PASS"',
         'qualification.get("status") == "TEST_ONLY_RAW_INPUTS_VERIFIED"')
    edit(service, 'pre = verified_preemission(request["preEmission"], base, release, request["toolchain"]["bend"]) if "preEmission" in request else None',
         'pre = hooks.preemission()  # TEST raw verifier; no release verifier or fallback')
    edit(service, 'shutil.copyfile(root / "EMISSION.json", out / "PRE-EMISSION.json")',
         'shutil.copyfile(root / "LOWMEM-EMISSION.json", out / "PRE-EMISSION.json")')
    edit(service, 'run(["scripts/build-js.sh", "src/server/main.bend", out / "backplane-serve"], logs / "js.log", env)',
         'hooks.link(["scripts/build-js.sh", "src/server/main.bend", out / "backplane-serve"], logs / "js.log", env)')
    edit(service, 'shutil.copyfile(root / "logs/entries.log", logs / "entries.log")',
         '(logs / "entries.log").write_text("TEST_ONLY: emission implicitly checked source; no stock entry verdict.\\n")')
    edit(service, 'shutil.copyfile(root / "PHASES.json", out / "PRE-EMISSION-PHASES.json")',
         'shutil.copyfile(root / "LOWMEM-PHASES.json", out / "PRE-EMISSION-PHASES.json")')
    edit(service, '    write(out / "BUILD.json", result)',
         '    result.update(hooks.provenance())\n    write(out / "BUILD.json", result)')
    edit(service, '    write(out / "SERVICE.json", result)',
         '    result.update(testOnly=True, releaseQualified=False, stockEquivalent=False)\n    write(out / "SERVICE.json", result)')
    edit(service, 'compiler = {"version": request["toolchain"]["bend"]["version"], "sha256": request["toolchain"]["bend"]["sha256"]}',
         'compiler = {"version": request["toolchain"]["bend"]["version"], "sha256": request["toolchain"]["bend"]["sha256"], "kind": "experimental-source-compiler-manifest", "releaseQualified": False}')
    edit('scripts/service-runtime.py', NORMAL, TEST)
    edit('desktop/produce-release.cjs', '/^VERIFIED_GENUINE/.test(build.status)',
         "(build.status==='TEST_ONLY_UNQUALIFIED_SERVICE_BUILD'&&build.testOnly===true&&build.releaseQualified===false)")
    edit('desktop/produce-release.cjs', "status:'STAGED_UNQUALIFIED',target,release:",
         "status:'STAGED_UNQUALIFIED',testOnly:true,releaseQualified:false,stockEquivalent:false,target,release:")
    edit('desktop/produce-release.cjs', 'const build={serviceVersion:VERSION,electronVersion:ELECTRON,sourceTree:release.tree,sourceCommit:release.commit,',
         "const build={testOnly:true,releaseQualified:false,stockEquivalent:false,buildProvenance:{compilerMode:'composite-settle-41262875',producerWorkId:'w-t1792701529-415584-1614985',releaseBuildApproved:false},serviceVersion:VERSION,electronVersion:ELECTRON,sourceTree:release.tree,sourceCommit:release.commit,")
    edit('desktop/produce-release.cjs', "status:'ASSEMBLED_FROM_BOUND_TARGET_ACCEPTANCE'",
         "status:'TEST_ONLY_ARCHIVE_WITH_BOUND_TARGET_EVIDENCE',releaseQualified:false,stockEquivalent:false")
    edit('desktop/produce-web-release.cjs', "source.status==='PASS'",
         "source.status==='TEST_ONLY_RAW_INPUTS_VERIFIED'")
    edit('desktop/produce-web-release.cjs', 'sourceQualified:true,releaseQualified:false',
         'sourceQualified:false,testOnly:true,stockEquivalent:false,releaseQualified:false')
    edit('desktop/produce-web-release.cjs', "status:'PACKAGED_FROM_QUALIFIED_WEB_ARTIFACT'",
         "status:'TEST_ONLY_PACKAGED_WEB_ARTIFACT',releaseQualified:false")
    # Native packaging still checks the real product's git identity. Its
    # stage/smoke/archive tools come from this isolated test tooling directory.
    windows = 'scripts/windows-existing-desktop-package.py'
    for file in ('prepare-native-stage.cjs', 'test/release-smoke.cjs', 'archive-after-smoke.cjs'):
        edit(windows, "product / 'desktop/" + file + "'",
             "Path(__file__).resolve().parents[1] / 'desktop/" + file + "'")
    # The unchanged build-js suffix preserves stamping, transform, private
    # bake cleanup, target/runtime flags, local-phase records and final move.
    build = (source / 'scripts/build-js.sh').read_text()
    suffix = build[build.index('# the stamps go into a copy,'):]
    prefix = '''#!/bin/sh
set -eu
cd "${BACKPLANE_TEST_PRODUCT:?TEST product path required}"
python3 scripts/sidebar-draft-guard.py --entry "$1"
# Separate TEST caller enforces the frozen admission/lock primitives.
src=$1
out=$2
bun=$(command -v bun)
now_ms() { echo $(( $(date +%s%N) / 1000000 )); }
work="build/js/$(basename "$out")"
mkdir -p "$work"
t0=$(now_ms)
python3 scripts/retained-js.py --actual-key "${BACKPLANE_PREEMITTED_KEY:?TEST binding key required}" --output "$work/all.js"
t1=$(now_ms)
how=test-only-retained-experimental
'''
    (out / 'scripts/compile-test-retained.sh').write_text(prefix + suffix)
    for name in ('raw_inputs.py', 'test_service_hooks.py'):
        shutil.copyfile(HERE / name, out / 'scripts' / name)
    # Parsing source is not execution of a producer or test campaign.
    ast.parse((out / service).read_text())
    ast.parse((out / 'scripts/service-runtime.py').read_text())
    result = {'schema': 1, 'status': 'PREPARED_TEST_TOOLS_NOT_EXECUTED',
        'sourceCommit': COMMIT, 'sourceTree': TREE, 'releaseQualified': False,
        'originalReleaseSourcesUnchanged': True,
        'replacements': edits,
        'members': {p.relative_to(out).as_posix(): sha(p) for p in sorted(out.rglob('*')) if p.is_file()},
        'originalProducerSHA256': sha(source / service),
        'originalStageSHA256': sha(source / 'desktop/produce-release.cjs'),
        'publication': 'HOLD_FOR_OUTPUT_REVIEW_AND_USER_CONFIRMATION'}
    (out / 'TEST-TOOLING.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result['status'], 'output': str(out), 'edits': len(edits)}))


if __name__ == '__main__':
    main()
