"""Protected-runner TEST target link from externally verified raw inputs only."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ('source', 'tools', 'inputs', 'output'):
        p.add_argument('--' + arg, type=Path, required=True)
    p.add_argument('--target', choices=['windows-x64', 'linux-x64', 'linux-arm64', 'darwin-arm64'], required=True)
    a = p.parse_args()
    source, tools, inputs, output = [x.resolve() for x in (a.source, a.tools, a.inputs, a.output)]
    sys.path.insert(0, str(tools / 'scripts'))
    bootstrap = load('test_bootstrap_input_transport', source / 'scripts/release-bootstrap.py')
    producer = load('test_only_service', tools / 'scripts/service-release.py')
    producer.ROOT = source
    commit = 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4'
    tree = bootstrap.source_identity(commit, source)
    bootstrap.verify(inputs, commit, tree, 'js')
    config = bootstrap.js_inputs(inputs, commit, tree)
    experimental = json.loads((inputs / 'TEST-RAW-INPUTS.json').read_text())
    need = producer.require
    need(experimental['testOnly'] is True and experimental['releaseQualified'] is False,
         'TEST packet cannot claim release qualification')
    refs = {name: producer.reference(inputs / producer.relative(experimental[name]))
            for name in ('receipt', 'toolchain', 'compilerArchive', 'worker')}
    # Validated descriptor: source compiler manifest, not an official executable.
    compiler = {**refs['toolchain'], 'version': 'composite-settle-41262875',
                'kind': 'source-compiler-manifest-not-an-official-executable'}
    bun = bootstrap.tool('bun', '--version')
    need(bun['version'] == bootstrap.BUN_VERSION, 'Pinned linker Bun required')
    runtime = bootstrap.bun_runtime(a.target, inputs.parent / ('bun-runtime-' + a.target))
    for dest, member in config.get('noticeInputs', {}).items():
        path = source / bootstrap.relative(dest)
        path.parent.mkdir(parents=True, exist_ok=True)
        need(not path.exists(), 'Notice destination exists; inspect existing attempt')
        shutil.copyfile(inputs / bootstrap.relative(member), path)
    request = {'schema': 1, 'kind': 'backplane-service-js-inputs', 'target': a.target,
        'release': {'version': '0.14.2', 'tag': 'v0.14.2', 'channel': 'release', 'commit': commit, 'tree': tree},
        'toolchain': {'bend': compiler, 'bun': bun, 'bunRuntime': runtime},
        'sourceQualification': producer.reference(inputs / config['sourceQualification']),
        'webNotices': producer.reference(inputs / config['webNotices']), 'experimentalInputs': refs}
    result = producer.js(request, inputs, output)
    record = producer.read(output / 'BUILD.json')
    need(record['status'] == 'TEST_ONLY_UNQUALIFIED_SERVICE_BUILD'
         and record['testOnly'] is True and record['releaseQualified'] is False,
         'TEST result must not become release provenance')
    print(json.dumps({**result, 'testOnly': True, 'releaseQualified': False}), flush=True)


if __name__ == '__main__':
    main()
