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


BASE_COMMIT = '94532cfb30a6a6b2579268424e03733c9b1cfe64'
BASE_TREE = '369bd65c2b2b1fa7baaa29e7bbde56adba32ba88'
HOTFIX_COMMIT = 'e564f904eb277ffbd8d0104d6d8afa90ff1aeb6f'
HOTFIX_TREE = '4b09684c1debc0ced67d4068679489f6ab11dd5d'
REVIEWED_DELTA = {'.github/workflows/release-bootstrap.yml': {'base': '8ffb5389c84d23efd7ca12969657cc560f1139c54a2cb0c40ce7e37d959c2d0d',
                                             'selected': '5482d4ada5008adf6c74abfe1ffb1eae09b2ecaba7458ff4669aba3cbe1d2b21'},
 '.github/workflows/release.yml': {'base': 'a288f0e2d0d356ad471637ff8a62f0e4eefe9f3b34f1b295f6e046035e6ec08f',
                                   'selected': '90c0b4cf24d61c25b4699820d424a98462bb7d82826ae010dba0848d8b7730c1'},
 '.github/workflows/windows-existing-desktop-package.yml': {'base': '16d8400b6912bbc240a17140c939543ef7ec760b6a7bd59a7e706aaff9eb6081',
                                                            'selected': '5954471024c868e2a5c3196caf7cfd9631ca421753d9633be614380ab48da429'},
 '.github/workflows/windows-standard-user.yml': {'base': None,
                                                 'selected': '794f3cb8d312c20719d1f341b008e5a360cbba3c87538cb49bc64bbadffaa94d'},
 'desktop/load-recovery.cjs': {'base': '3aa2ec19cfc5a99573eea11aaf53613ffbf15f81dcf6b44cbc81995389f8dc7e',
                               'selected': '50f96f2134c8f89aefd27e3e10fac14e0c922afe65e88ebd40cf42307565181c'},
 'desktop/main.cjs': {'base': 'd4ed2a0516d6f8c2919b9ffbcbb9cbbabdfc6982d9f8e50098e20212c0b66141',
                      'selected': 'af3d177f85c1d66e6e1c6dc0b067838c295d14d3e5b5ea37a0ff59d719249896'},
 'desktop/package-lock.json': {'base': '1466ca041b9eac20834aefb2da5c1f6a9fc85e6b0daba94870dc88264a74a450',
                               'selected': '4837c3db6f070b6a57b12130c2736d0748ab27cc76c6d6475dddb5cf103fe6ca'},
 'desktop/package.json': {'base': 'd5ab956a1962b91ddcd4f9f299a814600f58635c70974d453943e0969322b9be',
                          'selected': 'bf972e284dc2739cd9fcbd789d746206c2da6bdb6099255e168e49e15149cc4f'},
 'desktop/produce-release.cjs': {'base': '1e2dc47c07eb04f6cce7a4de26edf2823de5a381fc6fc1c799258b6c61561e63',
                                 'selected': '3a1f9a977fac7e83f19d6f9c1b13bf20dbd9fa0384fd359b7b4d164eb57ddcbe'},
 'desktop/produce-web-release.cjs': {'base': 'c2bf1ea5b494d07105020b6543dd36750c05efbceb9ab665e6f10330070c9b06',
                                     'selected': '98c3c993f23bc41a3d86018947cab2714333771b231d9a50b990573a2681e3a3'},
 'desktop/release-inputs.schema.json': {'base': '9032aabd43a5ed533f56a9ff37064c7b0951c000b8e8a1b697807415921b7430',
                                        'selected': '0c31936f03e13f1ea12811e3bcc9868a6e55aa29452cf043670ad54245cbdaaa'},
 'desktop/service.cjs': {'base': '8dff2e8d9149535cabe064f675df731fa8340d159a7dd4cdb3ddac9fd41d88fa',
                         'selected': '3c4c4da471bbbc991aecb412db62d004ab2ac43bb1c1fc75ca5e0653153e320b'},
 'desktop/startup.css': {'base': '02a01d9f732d9d1ccefd8c267a42573e61db2c72b2158559b1c56eccac29cf20',
                         'selected': 'bb06e18d492356e60a3b9ade032b4a7768116b93c8198c795eda46e610082daa'},
 'desktop/startup.html': {'base': '6b37d7c1713f04fb825bdb6f0270672af8b277447504d805046a002efd0eca57',
                          'selected': '990c0d96cbfdea8bff2db96b42568264001091bea50db7a96ebd34dbc1db6bdf'},
 'desktop/startup.js': {'base': '3d80978af9254823e6c6ce13f12695a7896fa39fff131957c95435934e4d293a',
                        'selected': '14ad29e51fcba7cb67c169262f19592b50cf3820807e6fa0ae683e6897c7533b'},
 'desktop/test/load-recovery.test.cjs': {'base': '949ddafb5ce844e4bf76c8d29da2ba53d8155a1f4feb7a7f85476a9a54c1a779',
                                         'selected': '805641f369c55a72830c52132e1c61f49ac1c67cc5d768776b9c2d1d22f46f2b'},
 'desktop/test/service-startup-error.test.cjs': {'base': None,
                                                 'selected': 'c5b8c78cf7b8aaced18a933a45aefb5d70a461aefd66b13b969c9d9e7d1a2f44'},
 'desktop/test/startup-recovery.test.cjs': {'base': '71c63201a28df6da276626aea6a5f29c32d1e7b84b2854a5e3a82ea573163b8d',
                                            'selected': '6e4e5ecf5c2e5d1e321cf858994fa69f0deeec57649b79ce80843c2f09ea6b0f'},
 'desktop/test/store-link.test.cjs': {'base': None,
                                      'selected': '6d4e2cd17671b5ad553cb8068a7bf65d9090798e661502c4962a8ab1f538cdfb'},
 'desktop/test/windows-standard-user.cjs': {'base': None,
                                            'selected': 'd3ad842e7427f1189aa40bfb076b871af37e1d3f0d8e62944a919835392c699f'},
 'scripts/release-bootstrap.py': {'base': '44aa04078431e67f6a48deb4e7aff0267e973d1f0d9d982230c86ebd469ab3bc',
                                  'selected': 'a77711568fc55c489323dfe5e1732248d16aee49c1aa9779f103abb686d8ccc7'},
 'scripts/service-release.py': {'base': '29ba06044c73b99c06176a70c2bba557c965798f368e4a686a7642971fc15ab4',
                                'selected': '86722ad1d13a307eb08815398c454619551f51fa157f4c0f91f0deb08348fb3c'},
 'scripts/windows-standard-user-smoke.ps1': {'base': None,
                                             'selected': '0d4237489839e8ed7a11d19f02e17ba1250c38ee809747f194892c8ce567da60'},
 'src/server/effects/storeio.js': {'base': '1719dd358a4a4533e702ca6ab66dbddd5c2b56f0970efaee28ceb33f658f5503',
                                   'selected': 'fc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93'}}

def source_equivalence(old, current):
    """Exact reviewed #907/version composition; no generic path exemptions."""
    changed = sorted(n for n in old.keys() | current.keys() if old.get(n) != current.get(n))
    if changed != sorted(REVIEWED_DELTA):
        raise ValueError('Source delta differs from the frozen #907/version composition')
    for name in changed:
        if (old.get(name), current.get(name)) != (REVIEWED_DELTA[name]['base'], REVIEWED_DELTA[name]['selected']):
            raise ValueError('Reviewed source bytes changed: ' + name)
    return changed


def previous_compile_inputs(producer):
    """Validate the old receipt as 0.14.0; never relabel its version fields."""
    producer.require(producer.VERSION == '0.14.1', 'Hotfix producer must be version0.14.1')
    result = copy.deepcopy(producer.compile_inputs())
    result['version'] = '0.14.0'
    result['versionStamp'] = '0.14.0'
    return result


def validate_previous_compile_inputs(producer, build):
    producer.require(build['compileInputs'] == previous_compile_inputs(producer),
        'Prior configured version/bake inputs mismatch')


def validate_raw_change(producer, emitted, prior_emit):
    producer.require(emitted['files']['server.js'] != prior_emit['files']['server.js'],
        'Changed storeio effect requires an actual new source-bound server emission')
    producer.require({k: v for k, v in emitted['files'].items() if k.startswith('web/')} ==
        {k: v for k, v in prior_emit['files'].items() if k.startswith('web/')}, 'Raw web changed')


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
    producer.require(a.source == HOTFIX_COMMIT and source == HOTFIX_TREE, 'Exact frozen hotfix required')
    bootstrap.verify(a.packet, a.source, source, 'js')
    expected = json.loads((a.packet / 'RELINK-EXPECTED.json').read_text())[a.target]
    old = a.previous / 'bootstrap-js'
    producer.require(producer.digest(old / 'BUILD.json') == expected['buildSha256'], 'Prior BUILD pin mismatch')
    producer.require(producer.digest(a.previous / 'bootstrap-service/component/SERVICE.json') ==
        expected['componentSha256'], 'Prior component pin mismatch')
    producer.require(producer.digest(a.previous / 'bootstrap-service/component/payload/RUNTIME-QUALIFICATION.json') ==
        expected['runtimeQualificationSha256'], 'Prior ABI qualification pin mismatch')
    producer.require(expected['sourceCommit'] == BASE_COMMIT and expected['sourceTree'] == BASE_TREE,
        'Reuse baseline must be the qualified frozen0.14.0 source')
    build = producer.read(old / 'BUILD.json')
    validate_previous_compile_inputs(producer, build)
    producer.require(build['status'] == 'VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD'
        and build['target'] == a.target and build['source_commit'] == expected['sourceCommit']
        and build['source_tree'] == expected['sourceTree'], 'Prior configured BUILD identity mismatch')
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
    validate_raw_change(producer, emitted, prior_emit)
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
        'rawEmission': 'NEW_ROOT_SERVER_EMISSION_UNCHANGED_WEB_REUSED', 'nativeABI': 'REUSED_PENDING_CHANGED_SERVICE_LAUNCH',
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
    producer.require(a.source == HOTFIX_COMMIT and b['source_commit'] == a.source
        and b['source_tree'] == HOTFIX_TREE and b['target'] == a.target
        and b['compileInputs'] == producer.compile_inputs(), 'New BUILD identity/version mismatch')
    producer.require(producer.digest(builddir / b['service']) == b['binary_sha256'], 'New binary changed')
    log = a.launch_log.read_text().strip()
    producer.require(log == 'backplane ' + producer.VERSION, 'Actual changed-service version launch missing')
    previous = a.previous / 'bootstrap-service'
    qualification = previous / 'component/payload/RUNTIME-QUALIFICATION.json'
    prior = producer.read(qualification)
    evidence = producer.read(builddir / 'logs/relink-provenance.json')
    producer.require(prior['status'] == 'PASS' and prior['target'] == a.target
        and prior['sourceCommit'] == evidence['previousSourceCommit']
        and prior['sourceTree'] == evidence['originalBuild']['source_tree']
        and prior['binarySha256'] == evidence['previousBinarySha256'], 'Prior ABI qualification mismatch')
    producer.require(producer.digest(qualification) ==
        evidence['priorRuntimeQualificationSha256'], 'Prior ABI qualification changed')
    runtime = copy_retained_runtime(producer, previous / 'component/payload',
                                    prior, a.out.parent / 'runtime-reuse')
    # CLOSURE edges start at the service filename, which deliberately is not
    # part of this runtime-only directory. The unchanged component producer
    # validates runtime_closure after combining it with the new service.
    approval = copy.deepcopy(prior)
    approval.update(sourceCommit=b['source_commit'], sourceTree=b['source_tree'],
        binarySha256=b['binary_sha256'],
        scope='Prior ABI evidence reused for identical native Bun runtime and native C effects; changed JS service/version launched on matching host',
        priorABIQualificationSha256=producer.digest(qualification),
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


def copy_retained_runtime(producer, payload, qualification, output):
    """Recover only the exact old runtime inventory from a qualified component."""
    producer.require(not output.exists(), 'Retain existing recovered runtime')
    output.mkdir(parents=True)
    for name, item in qualification['members'].items():
        relative = producer.relative(name)
        source, target = payload / relative, output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if item['type'] == 'file':
            producer.require(source.is_file() and not source.is_symlink()
                and producer.digest(source) == item['sha256']
                and source.stat().st_size == item['bytes'], 'Retained runtime member changed')
            shutil.copyfile(source, target)
            target.chmod(item['mode'])
        elif item['type'] == 'symlink':
            producer.require(source.is_symlink() and os.readlink(source) == item['target'],
                'Retained runtime link changed')
            target.symlink_to(item['target'])
        else:
            raise ValueError('Unsupported retained runtime member type')
    producer.require(producer.inventory(output) == qualification['members'], 'Prior runtime changed')
    return output


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
