#!/usr/bin/env python3
"""Relink an attested raw service while reusing exact accepted helper/ABI inputs.

The normal service producer still verifies the retained emission and performs
the configured transform/link. Only its two helper production calls are
replaced with verified copies. Packet files remain data, never executable code.
"""
import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def allowed_delta(name):
    return (name == 'scripts/js-native.py' or name.startswith(
        ('.github/', 'test/', 'tests/', 'docs/', 'build/ci-speed/proof-source-backend-review/'))
        or name == 'build/ci-speed/RESULTS.md')


def source_equivalence(old, current):
    changed = sorted(n for n in old.keys() | current.keys() if old.get(n) != current.get(n))
    if 'scripts/js-native.py' not in changed or any(not allowed_delta(n) for n in changed):
        raise ValueError('Reuse requires only native transform and CI/test/docs deltas')
    return changed


def helper_reuse(producer, old, build, original_run):
    """Narrow producer hooks: reject unknown helper commands, delegate all others."""
    def run(args, log, env, timeout=2400):
        if str(args[0]) != 'scripts/build-helpers.sh':
            return original_run(args, log, env, timeout)
        if len(args) != 2:
            raise ValueError('Unexpected helper production contract')
        out = Path(args[1])
        for name, expected in build['helpers'].items():
            producer.require(producer.digest(old / name) == expected, 'Reused helper mismatch')
            shutil.copy2(old / name, out / name)
            (out / name).chmod(0o755)
        if (old / 'licenses/helpers').exists():
            shutil.copytree(old / 'licenses/helpers', out / 'licenses/helpers')
        text = 'Exact helpers and helper notices copied from the bound prior BUILD; no helper compilation.\n'
        Path(log).write_text(text)
        return text
    return run


def prepare(a, producer, bootstrap):
    source = bootstrap.source_identity(a.source, a.checkout)
    bootstrap.verify(a.packet, a.source, source, 'js')
    expected = json.loads((a.packet / 'RELINK-EXPECTED.json').read_text())[a.target]
    old = a.previous / 'bootstrap-js'
    producer.require(producer.digest(old / 'BUILD.json') == expected['buildSha256'], 'Prior BUILD pin mismatch')
    producer.require(producer.digest(a.previous / 'bootstrap-service/component/SERVICE.json') ==
        expected['componentSha256'], 'Prior component pin mismatch')
    producer.require(producer.digest(a.previous / 'bootstrap-service/RUNTIME-QUALIFICATION.json') ==
        expected['runtimeQualificationSha256'], 'Prior ABI qualification pin mismatch')
    build = producer.read(old / 'BUILD.json')
    producer.require(build['status'] == 'VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD'
        and build['target'] == a.target and build['source_commit'] == expected['sourceCommit']
        and build['source_tree'] == expected['sourceTree']
        and build['compileInputs'] == producer.compile_inputs(), 'Prior configured BUILD identity mismatch')
    producer.require(producer.digest(old / build['service']) == build['binary_sha256']
        and producer.digest(old / 'SOURCE.json') == build['sourceManifestSha256'], 'Prior build member mismatch')
    old_source = producer.read(old / 'SOURCE.json')['members']
    current = {n: producer.digest(a.checkout / n) for n in producer.git('ls-files').splitlines()}
    changed = source_equivalence(old_source, current)
    inputs = bootstrap.js_inputs(a.packet, a.source, source)
    for dest, member in inputs.get('noticeInputs', {}).items():
        path = a.checkout / bootstrap.relative(dest)
        if path.exists():
            producer.require(bootstrap.sha(path) == bootstrap.sha(a.packet / member), 'Notice input differs')
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(a.packet / member, path)
    bun = bootstrap.tool('bun', '--version')
    bend = bootstrap.tool('bend', 'version')
    producer.require(bun['sha256'] == build['toolchain']['bun']['sha256']
        and bend['sha256'] == build['toolchain']['bend']['sha256'], 'Reused helper toolchain changed')
    runtime = bootstrap.bun_runtime(a.target, a.packet.parent / ('relink-bun-' + a.target))
    producer.require(runtime['sha256'] == build['toolchain']['bunRuntime']['sha256'], 'Native Bun runtime changed')
    request = {'schema': 1, 'kind': 'backplane-service-js-inputs', 'target': a.target,
        'release': {'version': producer.VERSION, 'tag': 'v' + producer.VERSION,
                    'channel': 'release', 'commit': a.source, 'tree': source},
        'toolchain': {'bend': bend, 'bun': bun, 'bunRuntime': runtime}}
    for key in ('preEmission', 'sourceQualification', 'webNotices'):
        request[key] = producer.reference(a.packet / bootstrap.relative(inputs[key]))
    # Verify raw identity before installing the strictly bounded reuse hooks.
    root, emitted = producer.verified_preemission(request['preEmission'], a.packet,
        request['release'], bend)
    prior_emit = producer.read(old / 'PRE-EMISSION.json')
    producer.require(emitted['files']['server.js'] == prior_emit['files']['server.js'], 'Raw server changed')
    producer.require({k: v for k, v in emitted['files'].items() if k.startswith('web/')} ==
        {k: v for k, v in prior_emit['files'].items() if k.startswith('web/')}, 'Raw web changed')
    return request, old, build, changed, expected


def build(a, producer, bootstrap):
    request, old, prior, changed, expected = prepare(a, producer, bootstrap)
    original_run = producer.run
    producer.run = helper_reuse(producer, old, prior, original_run)
    def kistack(out):
        producer.require(producer.check_kistack(old) == prior['kistack'], 'Reused KiStack changed')
        shutil.copytree(old / producer.KISTACK, out / producer.KISTACK)
    producer.build_kistack = kistack
    # js() keeps every normal source, secret, inventory, raw-emission, architecture
    # and output check. Its preEmission path has no emitter fallback.
    result = producer.js(request, a.packet, a.out)
    actual = producer.read(a.out / 'BUILD.json')
    producer.require(producer.read(old / 'WEB.json')['members'] ==
        producer.read(a.out / 'WEB.json')['members'], 'Transformed web bytes changed')
    producer.require(actual['helpers'] == prior['helpers'] and actual['kistack'] == prior['kistack'],
        'Unchanged helper/KiStack output mismatch')
    evidence = {'schema': 1, 'kind': 'configured-service-relink', 'sourceCommit': a.source,
        'sourceTree': request['release']['tree'], 'previousSourceCommit': prior['source_commit'],
        'previousBuildSha256': expected['buildSha256'], 'priorRun': expected['runId'],
        'changedSourceFiles': changed, 'helpers': 'EXACT_REUSE_NO_BUILD',
        'rawEmission': 'EXACT_REUSE_NO_BEND', 'nativeABI': 'REUSED_PENDING_CHANGED_SERVICE_LAUNCH',
        'previousBinarySha256': prior['binary_sha256'], 'binarySha256': actual['binary_sha256'],
        'originalBuild': prior, 'publication': False}
    evidence['priorRuntimeQualificationSha256'] = expected['runtimeQualificationSha256']
    producer.write(a.out / 'logs/relink-provenance.json', evidence)
    shutil.copyfile(__file__, a.out / 'EXECUTION-RELINK.py')
    actual['relink'] = producer.local_reference(a.out / 'logs/relink-provenance.json', a.out)
    actual['logs']['relink-provenance.json'] = actual['relink']
    # Existing component producer copies the execution helper. Keep the original
    # producer too, inside the normal copied evidence directory.
    shutil.copy2(a.out / 'EXECUTION-HELPER.py', a.out / 'logs/ORIGINAL-SERVICE-PRODUCER.py')
    shutil.copyfile(__file__, a.out / 'EXECUTION-HELPER.py')
    actual['executionHelper'] = producer.local_reference(a.out / 'EXECUTION-HELPER.py', a.out)
    producer.write(a.out / 'BUILD.json', actual)
    result['buildReceipt'] = producer.reference(a.out / 'BUILD.json')
    print(json.dumps(result))


def component(a, producer, bootstrap):
    # This phase runs only on the matching host after its actual version launch.
    host = {('Linux', 'x86_64'): 'linux-x64', ('Linux', 'aarch64'): 'linux-arm64',
            ('Darwin', 'arm64'): 'darwin-arm64', ('Windows', 'AMD64'): 'windows-x64'}
    producer.require(host.get((platform.system(), platform.machine())) == a.target, 'Matching host required')
    builddir = a.build
    b = producer.read(builddir / 'BUILD.json')
    producer.require(b['source_commit'] == a.source and b['target'] == a.target, 'New BUILD identity mismatch')
    producer.require(producer.digest(builddir / b['service']) == b['binary_sha256'], 'New binary changed')
    log = a.launch_log.read_text().strip()
    producer.require(log == 'backplane ' + producer.VERSION, 'Actual changed-service version launch missing')
    previous = a.previous / 'bootstrap-service'
    prior = producer.read(previous / 'RUNTIME-QUALIFICATION.json')
    evidence = producer.read(builddir / 'logs/relink-provenance.json')
    producer.require(prior['status'] == 'PASS' and prior['target'] == a.target
        and prior['sourceCommit'] == evidence['previousSourceCommit']
        and prior['sourceTree'] == evidence['originalBuild']['source_tree']
        and prior['binarySha256'] == evidence['previousBinarySha256'], 'Prior ABI qualification mismatch')
    producer.require(producer.digest(previous / 'RUNTIME-QUALIFICATION.json') ==
        evidence['priorRuntimeQualificationSha256'], 'Prior ABI qualification changed')
    runtime = previous / 'runtime'
    # Artifact transport drops modes; restore only the modes bound in the old
    # qualification before verifying that unchanged runtime inventory.
    for name, item in prior['members'].items():
        p = runtime / producer.relative(name)
        if item['type'] == 'file':
            p.chmod(item['mode'])
    producer.require(producer.inventory(runtime) == prior['members'], 'Prior runtime changed')
    producer.runtime_closure(runtime, a.target)
    approval = copy.deepcopy(prior)
    approval.update(sourceCommit=b['source_commit'], sourceTree=b['source_tree'],
        binarySha256=b['binary_sha256'],
        scope='Prior ABI evidence reused for identical native Bun runtime/effects; changed service launched on matching host',
        priorABIQualificationSha256=producer.digest(previous / 'RUNTIME-QUALIFICATION.json'),
        matchingHostLaunch=producer.reference(a.launch_log))
    approval_path = a.out.parent / 'RELINK-RUNTIME-QUALIFICATION.json'
    a.out.parent.mkdir(parents=True, exist_ok=True)
    producer.write(approval_path, approval)
    request = {'schema': 1, 'kind': 'backplane-service-component-inputs', 'target': a.target,
        'release': {'version': producer.VERSION, 'tag': 'v' + producer.VERSION, 'channel': 'release',
                    'commit': a.source, 'tree': b['source_tree']},
        'buildReceipt': producer.reference(builddir / 'BUILD.json'),
        'buildDirectory': str(builddir.resolve()), 'runtimeDirectory': str(runtime.resolve()),
        'runtimeQualification': producer.reference(approval_path)}
    print(json.dumps(producer.component(request, a.out.parent, a.out)))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase', choices=['build', 'component'])
    p.add_argument('--checkout', required=True, type=Path)
    p.add_argument('--source', required=True)
    p.add_argument('--target', required=True)
    p.add_argument('--previous', required=True, type=Path)
    p.add_argument('--packet', type=Path)
    p.add_argument('--build', type=Path)
    p.add_argument('--launch-log', type=Path)
    p.add_argument('--out', required=True, type=Path)
    a = p.parse_args()
    a.checkout = a.checkout.resolve(); a.out = a.out.resolve(); a.previous = a.previous.resolve()
    toolroot = Path(__file__).resolve().parent
    producer = load('service_release', toolroot / 'service-release.py')
    bootstrap = load('release_bootstrap', toolroot / 'release-bootstrap.py')
    producer.ROOT = a.checkout
    bootstrap.source_identity(a.source, a.checkout)
    if a.phase == 'build':
        producer.require(a.packet is not None, 'Packet required')
        a.packet = a.packet.resolve()
        build(a, producer, bootstrap)
    else:
        producer.require(a.build is not None and a.launch_log is not None, 'Build and launch log required')
        component(a, producer, bootstrap)


if __name__ == '__main__':
    main()
