#!/usr/bin/env python3
"""Run existing source-bound desktop stages on matching GUI hosts; never build."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import urllib.parse
import urllib.request

SOURCE = '8147b68dbc9bcddad7e3b51ae7768ed9eb211ecc'
TREE = '37c64023d084bca7f209db9d14a84f59d365f696'
SOURCE_PACKET = '43241fdafe1268a82301caff6a6f0370adfd4b6b10172b28df8d1c91b2c4a168'
POLICY = '0146a1527cd121ed04c8f36052ba0f31232176e56910387cb84f0dc97d7c4ef2'
SMOKE = '0d268b94bf2b5b6b8722e5a17a01fe13447cb4f034de363ca75566b4449297b6'
PRODUCER = 'f2ead45db28612c7f744a1b4da453f6b861f2a6e5ad0f8a8363caec016a2fb89'
IO = 'b449d3a77f320ea865924637c662c5de80f74093232188160844837f3a3f6489'
BINARIES = {'linux-x64': '9d48859d2cf2d096d63a1d365f077a045255cd5ea15a2b892ed77d23c09ace1e',
            'darwin-arm64': '2d7bdcab9b4217e697e3dfecff87cf34574d180440b868e63fddb75c684aef2a'}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    with Path(path).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def write(path, record):
    path.write_text(json.dumps(record, indent=2)+'\n')


def fetch(url, expected, dest, limit):
    u = urllib.parse.urlsplit(url)
    require(u.scheme == 'https' and u.hostname and not u.username and not u.password
            and not u.fragment and u.port in (None, 443), 'Public HTTPS input required')
    require(re.fullmatch('[0-9a-f]{64}', expected) and not dest.exists(), 'Fresh hash-bound input required')
    with urllib.request.urlopen(url, timeout=120) as response, dest.open('xb') as out:
        require(urllib.parse.urlsplit(response.url).scheme == 'https', 'Insecure redirect refused')
        count = 0
        for block in iter(lambda: response.read(1024*1024), b''):
            count += len(block)
            require(count <= limit, 'Input size limit')
            out.write(block)
    require(sha(dest) == expected, 'Input SHA mismatch')


def preflight(target, out):
    actual = {('Linux', 'x86_64'): 'linux-x64', ('Darwin', 'arm64'): 'darwin-arm64'}.get(
        (platform.system(), platform.machine()))
    require(actual == target, 'Matching native host required')
    if target == 'linux-x64':
        version = platform.libc_ver()
        require(version[0] == 'glibc' and tuple(map(int, version[1].split('.'))) >= (2, 38),
                'Original service requires GLIBC 2.38 or newer')
        argv = ['/usr/bin/unshare', '--user', '--map-root-user', '/usr/bin/true']
    else:
        version = None
        argv = ['/bin/launchctl', 'print', 'gui/'+str(os.getuid())]
    r = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=10, close_fds=True)
    (out/'preflight.stdout').write_bytes(r.stdout)
    (out/'preflight.stderr').write_bytes(r.stderr)
    record = {'target': target, 'host': platform.platform(), 'libc': version,
              'argv': argv, 'exit': r.returncode, 'systemChanges': False,
              'sandboxBypass': False, 'payloadExecuted': False}
    write(out/'PREFLIGHT.json', record)
    require(r.returncode == 0, 'Required native GUI/user namespace host capability unavailable')


def run(args):
    require(args.target in BINARIES and args.out.is_absolute() and not args.out.exists(),
            'Fresh supported target output required')
    args.out.mkdir(parents=True)
    preflight(args.target, args.out)
    manifest_path = args.out/'INPUTS.json'
    fetch(args.request_url, args.request_sha256, manifest_path, 65536)
    request = json.loads(manifest_path.read_text())
    require((request.get('schema'), request.get('sourceCommit'), request.get('sourceTree'),
             request.get('version'), request.get('channel')) == (1, SOURCE, TREE, '0.12.0', 'release'),
            'Root frozen desktop input identity mismatch')
    leaf = request['targets'][args.target]
    io_path = Path(__file__).resolve().parents[1]/'desktop/release-files.py'
    require(sha(io_path) == IO, 'Existing offline archive IO changed')
    spec = importlib.util.spec_from_file_location('release_archive_io', io_path)
    io = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(io)
    extracted = {}
    for name, ref, limit in [('source', request['sourcePacket'], 10*1024**2),
                              ('tools', request['playwrightTools'], 100*1024**2),
                              ('acceptance', request['acceptanceTools'], 10*1024**2),
                              ('stage', leaf['stage'], 1024**3)]:
        if name == 'source':
            require(ref['sha256'] == SOURCE_PACKET, 'Canonical source packet required')
        archive = args.out/(name+'.tar.gz')
        fetch(ref['url'], ref['sha256'], archive, limit)
        require(archive.stat().st_size == ref['bytes'], 'Input size mismatch')
        dest = args.out/(name+'-input')
        io.extract(archive, dest)
        extracted[name] = dest
    source = extracted['source']/'native-stage-tools'
    stage = extracted['stage']/'stage'
    require(sha(source/'root-policy.json') == POLICY and sha(source/'desktop/test/release-smoke.cjs') == SMOKE
            and sha(source/'desktop/produce-release.cjs') == PRODUCER, 'Canonical essential tools/policy changed')
    staged = json.loads((stage/'STAGE.json').read_text())
    require(sha(stage/'STAGE.json') == leaf['stageReceiptSha256']
            and staged['payloadManifestSha256'] == leaf['payloadManifestSha256']
            and (staged['target'], staged['release']['commit'], staged['release']['tree'],
                 staged['release']['version'], staged['release']['channel']) ==
            (args.target, SOURCE, TREE, '0.12.0', 'release'), 'Actual original stage identity mismatch')
    binaries = [p for p in stage.rglob('backplane-serve') if p.is_file()]
    require(len(binaries) == 1 and sha(binaries[0]) == BINARIES[args.target], 'Original configured binary changed')
    packages = list(extracted['tools'].rglob('playwright-core/package.json'))
    require(len(packages) == 1 and json.loads(packages[0].read_text())['version'] == '1.64.1',
            'Exact Playwright 1.64.1 required')
    acceptance = extracted['acceptance']/'acceptance-tools/archive-after-smoke.cjs'
    require(sha(acceptance) == request['acceptanceTools']['entrySha256'], 'Actual acceptance adapter changed')
    env = {k:v for k,v in os.environ.items() if not re.search(
        r'TOKEN|API_KEY|SECRET|PASSWORD|CREDENTIAL|AUTH_COOKIE', k)
        and not k.startswith(('LD_', 'DYLD_', 'BACKPLANE_GOOGLE_', 'BACKPLANE_TELEMETRY_'))}
    env.update(BACKPLANE_TELEMETRY='off', BACKPLANE_DESKTOP_BROWSER='1',
               NODE_PATH=str(packages[0].parent.parent))
    node = subprocess.check_output(['node', '-p', 'process.execPath'], text=True).strip()
    require(subprocess.check_output([node, '--version'], text=True).strip() == 'v22.22.3',
            'Pinned native Node version required')
    execution = {'target': args.target, 'sourceCommit': SOURCE, 'sourceTree': TREE,
                 'metadataCommit': os.environ.get('GITHUB_SHA'), 'helperSha256': sha(__file__),
                 'requestSha256': args.request_sha256, 'stageReceiptSha256': sha(stage/'STAGE.json'),
                 'originalBinarySha256': sha(binaries[0]), 'smokeSha256': SMOKE,
                 'canonicalPolicySha256': POLICY, 'telemetryValidation': 'DEFERRED_BY_USER',
                 'telemetryRuntimeOptOut': True, 'compilerInvoked': False, 'published': False}
    write(args.out/'EXECUTION.json', execution)
    evidence = args.out/'evidence'
    argv = [node, str(source/'desktop/test/release-smoke.cjs'), '--stage', str(stage), '--out', str(evidence)]
    if args.target == 'linux-x64':
        argv = ['/usr/bin/xvfb-run', '--auto-servernum', '--server-args=-screen 0 1280x900x24 -nolisten tcp', *argv]
    with (args.out/'smoke.stdout').open('wb') as stdout, (args.out/'smoke.stderr').open('wb') as stderr:
        child = subprocess.Popen(argv, env=env, stdin=subprocess.DEVNULL, stdout=stdout,
                                 stderr=stderr, close_fds=True, start_new_session=True)
        write(args.out/'SUPERVISOR.json', {'pid': child.pid, 'ownedSession': True, 'argv': argv})
        try:
            rc = child.wait(timeout=180)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGTERM)
            child.wait(timeout=10)
            raise ValueError('Bounded actual desktop smoke timed out; no acceptance')
    require(rc == 0, 'Actual desktop smoke failed; inspect preserved raw evidence')
    require(sha(stage/'STAGE.json') == leaf['stageReceiptSha256'], 'Stage receipt mutated')
    cp = subprocess.run([node, str(acceptance), '--producer', str(source/'desktop/produce-release.cjs'),
                         '--stage', str(stage), '--evidence', str(evidence),
                         '--out', str(args.out/'acceptance'), '--dist', str(args.out/'dist')],
                        env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=180, close_fds=True)
    (args.out/'archive.stdout').write_bytes(cp.stdout)
    (args.out/'archive.stderr').write_bytes(cp.stderr)
    require(cp.returncode == 0, 'Strict archive-after-smoke adapter refused acceptance')
    write(args.out/'RESULT.json', {**execution, 'status': 'ACTUAL_ESSENTIAL_RUNTIME_AND_ARCHIVE_PASS',
                                  'evidence': str(evidence), 'dist': str(args.out/'dist')})
    print(json.dumps({'status': 'ACTUAL_ESSENTIAL_RUNTIME_AND_ARCHIVE_PASS', 'target': args.target,
                      'dist': str(args.out/'dist'), 'published': False}), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--request-url', required=True)
    p.add_argument('--request-sha256', required=True)
    p.add_argument('--target', required=True)
    p.add_argument('--out', type=Path, required=True)
    run(p.parse_args())


if __name__ == '__main__':
    main()
