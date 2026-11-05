"""Source-bound admission for the explicitly authorized 0142 recut.

This module verifies data only. It does not invoke Bend, a linker, a workflow,
or the official verified_preemission path, and never rewrites a producer receipt.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import policy_checks

OLD_SOURCE = 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4'
OLD_WORK = 'w-t1792701529-415584-1614985'
FIX950 = 'e07633b1203b27051d65ef9071816eea5acfea89'
COMPILER = {
    'mode': 'composite-settle-41262875',
    'manifestSha256': '29d715ad67c69722f67b916fd35ce286160f88ec382fe92748840a1943cf4006',
    'archiveSha256': 'be39bfb8c44da887702c51ef80dea07a6b0366c754f2eb15615e73a1f0d62581',
    'emitterBunSha256': '795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822',
    'flagsWorkerSha256': '141b854565f7370fb2b959ef545d9255824b3487f73ab0295829b3216000dabd',
    'sourceFileCount': 125,
}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def regular(path):
    p = Path(path).absolute()
    require(p.is_file() and not any(x.is_symlink() for x in (p, *p.parents)),
            'Regular non-symlink input required: ' + str(p))
    return p


def reference(path):
    p = regular(path)
    return {'path': str(p), 'sha256': digest(p), 'bytes': p.stat().st_size}


def safe_member(name):
    require(isinstance(name, str) and name and str(PurePosixPath(name)) == name
            and not PurePosixPath(name).is_absolute() and '\\' not in name and ':' not in name
            and all(x not in ('', '.', '..') for x in name.split('/')), 'Unsafe member')
    return name


def verify(ready_path, ready_sha, source, selection_path, selection_sha,
           authority_path, authority_sha, compiler_manifest, compiler_archive, worker, receipt_path=None):
    ready_path, selection_path, authority_path, compiler_manifest, compiler_archive, worker = map(
        regular, (ready_path, selection_path, authority_path, compiler_manifest, compiler_archive, worker))
    require(digest(ready_path) == ready_sha and digest(selection_path) == selection_sha
            and digest(authority_path) == authority_sha, 'Pinned root handoff/authority differs')
    ready, selection, authority = map(read, (ready_path, selection_path, authority_path))
    # Consume the actual root receipt format; a local plan is not authority.
    required = authority['requiredSource']
    require(required.get('mainAfterPR') == 951 and required.get('includePR950') == FIX950
            and required.get('excludePR947')
            and set(required.get('includeAdditionalPRs', [])) == {954, 928, 923, 922}, 'Wrong root source authority')
    consent = authority.get('lawRemovalConsent', {})
    require(consent.get('state') == 'EXPLICIT_USER_APPROVED_VIA_BUB'
            and set(consent.get('laws', [])) == {'cad_install_visible_action', 'cad_install_monotonic'},
            'Root exact two-law consent not recorded')
    compiler_policy, packaging, publication = authority['compiler'], authority['packaging'], authority['publication']
    require(compiler_policy.get('mode') == COMPILER['mode'] and compiler_policy.get('hubWebCompileCount') == 1
            and compiler_policy.get('hardMemoryCapBytes') == 8589934592
            and compiler_policy.get('swapMaxBytes') == 0 and compiler_policy.get('newStockReference') is False
            and compiler_policy.get('XLARGE') is False, 'Wrong root compiler policy')
    require(packaging.get('parallel') is True and packaging.get('owner') == 't1792726353-440155'
            and packaging.get('transport') == 'AUTHENTICATED_GITHUB_ARTIFACTS_ONLY'
            and set(packaging.get('targets', [])) == {'linux-x64', 'linux-arm64', 'darwin-arm64',
                'windows-x64', 'web', 'android', 'ios'}, 'Wrong packaging authority')
    require(publication.get('state') == 'PREAUTHORIZED_AFTER_REQUIRED_WINDOWS_PASS_AND_ALL_PLATFORM_PACKAGES'
            and publication.get('solePublisher') == 't1792701529-415584'
            and publication.get('tag') == 'v0.14.2', 'Wrong conditional publication authority')
    commit, tree = ready['sourceCommit'], ready['sourceTree']
    require(re.fullmatch('[0-9a-f]{40}', commit) and re.fullmatch('[0-9a-f]{40}', tree)
            and commit != OLD_SOURCE, 'Fresh recut source required; old afa cannot be relabeled')
    require((authority.get('sourceCommit'), authority.get('sourceTree')) == (commit, tree),
            'Root authority not yet bound to actual frozen source')
    work = ready['work']
    require(work.startswith('w-t1792701529-415584-') and work != OLD_WORK,
            'New actual root-owned shared emitter required')
    require((ready['version'], ready['channel']) == ('0.14.2', 'release'), 'Wrong version/channel')
    source = Path(source).resolve(strict=True)
    def git(*args):
        return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()
    require((git('rev-parse', 'HEAD'), git('rev-parse', 'HEAD^{tree}')) == (commit, tree)
            and not git('status', '--porcelain', '--untracked-files=no'), 'Frozen source differs/dirty')
    require(selection.get('sourceCommit') == commit and selection.get('sourceTree') == tree,
            'Root source-selection receipt differs')
    policy_checks.selection(selection, commit, tree)
    merged = selection.get('normalMergedCommits', {})
    require(merged.get('950') == FIX950 and all(re.fullmatch('[0-9a-f]{40}', merged.get(n, '')) for n in ('948', '951')),
            'Normally merged948/950/951 source selection required')
    for revision in (FIX950, *(merged[str(n)] for n in (948, 951, 954, 928, 923, 922))):
        require(subprocess.run(['git', '-C', str(source), 'merge-base', '--is-ancestor', revision, commit],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0,
                'Required fix merge is not in the frozen candidate')
    require({932, 947} <= set(selection.get('excludedPRs', [])), 'Both932 and947 must be excluded')
    consent_record = selection['lawRemoval']
    require(digest(regular(selection_path.parent / consent_record['receipt'])) == consent_record['sha256'],
            'Explicit law consent receipt differs')
    qualification = selection['fresh24']
    require(qualification.get('verdict') == 'PASS' and qualification.get('passed') == 24
            and qualification.get('total') == 24 and qualification.get('sourceTree') == tree
            and qualification.get('inherited') is False, 'Fresh24/24 on exact excluded tree required')
    proof = regular(selection_path.parent / qualification['receipt'])
    require(digest(proof) == qualification['sha256'], 'Fresh24 receipt differs')
    policy_checks.source_identity(read(proof), commit, tree)
    review = selection['pinManifestReview']
    review_path = regular(selection_path.parent / review['receipt'])
    manifest_path = regular(selection_path.parent / review['manifest'])
    require(digest(review_path) == review['sha256'] and digest(manifest_path) == review['manifestSha256'],
            'Actual pin/manifest review bytes differ')
    policy_checks.source_identity(read(review_path), commit, tree)
    require(digest(compiler_manifest) == COMPILER['manifestSha256']
            and digest(compiler_archive) == COMPILER['archiveSha256']
            and digest(worker) == COMPILER['flagsWorkerSha256'], 'Actual compiler/worker identity differs')
    tools = read(compiler_manifest)
    require(tools['preferredcompiler_mode'] == COMPILER['mode']
            and tools['bun_binary_sha256'] == COMPILER['emitterBunSha256'], 'Wrong source compiler/Bun')
    pins = tools['compiler_files']
    require(len(pins) == COMPILER['sourceFileCount'], 'All125 compiler files required')
    actual = {}
    with tarfile.open(compiler_archive) as archive:
        for item in archive.getmembers():
            if item.isdir():
                continue
            require(item.isfile() and not item.issparse(), 'Compiler links/special files refused')
            name = safe_member(item.name)
            require(name.startswith('compiler/'), 'Wrong compiler archive root')
            name = name.removeprefix('compiler/')
            require(name not in actual, 'Duplicate compiler member')
            actual[name] = hashlib.file_digest(archive.extractfile(item), 'sha256').hexdigest()
    require(actual == pins, 'Compiler file inventory differs')
    # Authenticated packets relocate bytes; the authoritative original receipt
    # SHA, not its coordinator's absolute pathname, controls that relocation.
    receipt = regular(receipt_path if receipt_path is not None else ready['receipt'])
    require(digest(receipt) == ready['receiptSHA256'], 'Actual collected raw receipt differs')
    raw, root = read(receipt), receipt.parent
    require(raw.get('kind') == 'backplane-experimental-source-compiler-emission'
            and raw.get('sourceCommit') == commit and raw.get('sourceTree') == tree
            and raw.get('version') == '0.14.2' and raw.get('channel') == 'release'
            and raw.get('status') == 'COMPLETE_RAW_BYTES_STOCK_COMPARISON_REQUIRED'
            and raw.get('compiler') == tools and raw.get('releaseQualified') is False,
            'New successful source-compiler receipt required')
    members = {}
    for p in sorted(root.rglob('*')):
        require(not p.is_symlink(), 'Raw links refused')
        if p.is_file() and p != receipt:
            members[safe_member(p.relative_to(root).as_posix())] = {'sha256': digest(p), 'bytes': p.stat().st_size}
    require(members == raw['files'], 'Complete raw inventory mismatch')
    require(read(root/'SOURCE.json') == {'commit': commit, 'tree': tree,
            'members': {n: digest(source/n) for n in git('ls-files').splitlines()}}, 'Complete source differs')
    phases = read(root/'LOWMEM-PHASES.json')
    require([x.get('phase') for x in phases] == ['server', 'web'], 'Both shared phases required')
    for phase in phases:
        require(phase.get('exitCode') == 0 and phase.get('hardMemoryCapBytes') == 8589934592
                and phase.get('swapMaxBytes') == 0 and phase.get('seconds', 0) > 0
                and phase.get('peakRSSKiB', 0) > 0 and phase.get('sourceCommit') == commit
                and phase.get('sourceTree') == tree
                and phase.get('compilerArchiveSHA256') == COMPILER['archiveSha256'],
                'Actual capped phase metrics/verdict missing')
        end = read(root/'logs'/(phase['phase']+'.EXIT.json'))
        policy_checks.phase(phase, end, commit, tree, COMPILER['archiveSha256'])
        events = dict(line.split() for line in end['memoryEvents'].splitlines())
        require(end['exitCode'] == 0 and end['compilerPostIdentity'] is True
                and all(int(events.get(k, -1)) == 0 for k in ('oom', 'oom_kill', 'oom_group_kill')),
                'Compiler exit/postidentity/OOM evidence failed')
    require(regular(root/'server.js').stat().st_size > 0 and regular(root/'web/index.html').stat().st_size > 0
            and list((root/'web').glob('chunk-*.js')), 'Actual shared outputs missing')
    binding = {'schema': 1, 'kind': 'authorized-source-compiler-release-input-binding',
        'candidate': '0.14.2-recut951', 'sourceCommit': commit, 'sourceTree': tree,
        'version': '0.14.2', 'channel': 'release', 'producerWorkId': work,
        'rootReady': reference(ready_path), 'sourceSelection': reference(selection_path),
        'authority': reference(authority_path), 'rawReceipt': reference(receipt),
        'rawServer': reference(root/'server.js'), 'compilerManifest': reference(compiler_manifest),
        'compilerArchive': reference(compiler_archive), 'flagsWorker': reference(worker),
        'compilerMode': COMPILER['mode'], 'compilerFiles': pins,
        'emitterBunSha256': COMPILER['emitterBunSha256'], 'rawWebMembers': {
            n: v for n, v in members.items() if n.startswith('web/')},
        'rawHistoricalReleaseQualified': raw.get('releaseQualified'),
        'releaseUseAuthorized': True, 'stockByteEquivalent': False,
        'byteQualification': 'NO_CURRENT_SOURCE_STOCK_COMPARISON_CARRIED_OVER_OPTION2_AFA96792',
        'stockComparison': 'Earlier Option 2 disposition on afa96792 carried over to the approved recut; reviewed bytes are the 518 reference only. No stock reference exists for this exact source. This is not a new person choice about comparison for the new source.',
        'reviewedByteReference': '518 reference only',
        'currentSourceStockReferenceExists': False,
        'fullStockByteEquivalenceClaimAllowed': False,
        'platformQualification': 'PENDING actual new packaged gates',
        'officialVerifierChanged': False, 'fallbackEmissionAllowed': False}
    binding['bindingKey'] = hashlib.sha256(json.dumps(binding, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return binding
