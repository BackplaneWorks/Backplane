"""Read one explicitly authorized unpublished draft asset by immutable API IDs.

No write endpoint, release publication, arbitrary URL, extraction or executable
input is supported. GITHUB_TOKEN remains an environment-only workflow token.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

REPO = 'BackplaneWorks/Backplane'
SOURCE = 'be904cd4cb21ad6febe17e674918c96945113d07'
MAX_BYTES = 256 * 1024**2


def require(ok, message):
    if not ok:
        raise ValueError(message)


def validate(release, asset, release_id, asset_id, digest, size, name):
    require(release.get('id') == release_id and release.get('draft') is True
            and release.get('published_at') is None, 'Exact release is not an unpublished draft')
    require(release.get('target_commitish') == SOURCE, 'Transport draft targets a different source')
    require(asset.get('id') == asset_id and asset.get('name') == name
            and asset.get('state') == 'uploaded' and asset.get('size') == size,
            'Exact asset identity/name/size/state differs')
    require(any(x.get('id') == asset_id for x in release.get('assets', [])),
            'Asset does not belong to the exact unpublished draft')
    require(asset.get('digest') in (None, 'sha256:'+digest), 'GitHub asset digest differs')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ('release-id', 'asset-id', 'bytes'):
        p.add_argument('--'+arg, type=int, required=True)
    for arg in ('sha256', 'name', 'out'):
        p.add_argument('--'+arg, required=True)
    a = p.parse_args()
    out = Path(a.out).absolute()
    require(not out.exists(), 'Preserve existing intake directory; no silent retry')
    out.mkdir(parents=True)
    result = {'schema': 1, 'status': 'INCOMPLETE', 'repository': REPO,
        'sourceCommit': SOURCE, 'releaseId': a.release_id, 'assetId': a.asset_id,
        'expectedSha256': a.sha256, 'expectedBytes': a.bytes,
        'workflowPermissions': {'contents': 'read'}, 'permissionChanged': False,
        'audience': 'User accepted signed-in public-repository Actions readers; not private storage',
        'draftCleanupOwner': 't1792701529-415584', 'temporaryArtifactCleanupRequired': True,
        'releasePublicationSupported': False, 'startedAt': time.time()}

    def api(suffix, binary=None):
        args = ['gh', 'api', '--method', 'GET',
                '-H', 'Accept: '+('application/octet-stream' if binary else 'application/vnd.github+json'),
                '-H', 'X-GitHub-Api-Version: 2022-11-28', 'repos/'+REPO+suffix]
        if binary:
            with binary.open('xb') as stream:
                r = subprocess.run(args, stdout=stream, stderr=subprocess.PIPE, timeout=180)
        else:
            r = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        if r.returncode:
            match = re.search(rb'HTTP\s+(\d{3})', r.stderr)
            result['httpFailure'] = int(match[1]) if match else None
            result['failedEndpoint'] = suffix
            raise RuntimeError('Existing contents:read workflow token could not read the exact draft/asset; preserve evidence and report before requesting any permission change')
        return None if binary else json.loads(r.stdout)

    try:
        require(a.release_id > 0 and a.asset_id > 0 and 0 < a.bytes <= MAX_BYTES,
                'Invalid immutable ID or bounded size')
        require(re.fullmatch('[0-9a-f]{64}', a.sha256) and re.fullmatch('[A-Za-z0-9._-]+', a.name)
                and a.name.endswith('.tar.gz'), 'Invalid pinned SHA/name')
        require(os.environ.get('GH_TOKEN'), 'Workflow token unavailable')
        group = next(x.split(':', 2)[2] for x in Path('/proc/self/cgroup').read_text().splitlines() if x.startswith('0::'))
        cg = Path('/sys/fs/cgroup')/group.lstrip('/')
        cap = (cg/'memory.max').read_text().strip()
        require(cap.isdigit() and 0 < int(cap) <= 536870912
                and (cg/'memory.swap.max').read_text().strip() == '0', 'Actual intake512MiB/swap0 scope missing')
        result['kernelMemoryMaxBytes'] = int(cap)
        result['kernelSwapMaxBytes'] = 0
        result['phase'] = 'EXISTING_READ_TOKEN_DRAFT_CANARY'
        release = api('/releases/'+str(a.release_id))
        asset = api('/releases/assets/'+str(a.asset_id))
        validate(release, asset, a.release_id, a.asset_id, a.sha256, a.bytes, a.name)
        result['draftReadCanary'] = 'PASS'
        result['phase'] = 'AUTHENTICATED_ASSET_ID_DOWNLOAD'
        payload = out/'seed-inputs.tar.gz'
        api('/releases/assets/'+str(a.asset_id), payload)
        require(payload.stat().st_size == a.bytes, 'Downloaded asset size differs')
        with payload.open('rb') as stream:
            actual = hashlib.file_digest(stream, 'sha256').hexdigest()
        require(actual == a.sha256, 'Downloaded asset SHA differs')
        # Recheck the draft state before exposing bytes as a temporary artifact.
        validate(api('/releases/'+str(a.release_id)), asset,
                 a.release_id, a.asset_id, a.sha256, a.bytes, a.name)
        result.update(status='PASS_AUTHENTICATED_DRAFT_ASSET_READ_AND_HASH',
                      downloadedSha256=actual, downloadedBytes=a.bytes,
                      payload=payload.name, draftRemainsUnpublished=True)
    except Exception as e:
        result.update(status='FAIL', error=str(e))
        raise
    finally:
        result['finishedAt'] = time.time()
        (out/'INTAKE.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
