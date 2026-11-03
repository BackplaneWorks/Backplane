#!/usr/bin/env python3
"""Exercise shallow/full/blobless Git checkout semantics on a disposable fixture.

Uses file:// upload-pack with filtering enabled, never local clone hardlinks.
Times are local transport measurements, not GitHub network/Actions timings.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time


def git(cwd, *args, check=True):
    return subprocess.run(
        ['git', '-c', 'protocol.file.allow=always', *args], cwd=cwd,
        env={**os.environ, 'GIT_CONFIG_NOSYSTEM': '1',
             'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_TERMINAL_PROMPT': '0',
             'GIT_AUTHOR_NAME': 'Fixture', 'GIT_COMMITTER_NAME': 'Fixture',
             'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
             'GIT_COMMITTER_EMAIL': 'fixture@example.invalid',
             'GIT_AUTHOR_DATE': '2026-01-01T00:00:00Z',
             'GIT_COMMITTER_DATE': '2026-01-01T00:00:00Z'},
        text=True, capture_output=True, check=check, timeout=15)


def inventory(root):
    return {p.relative_to(root).as_posix(): {
        'sha256': hashlib.sha256(p.read_bytes()).hexdigest(),
        'executable': bool(p.stat().st_mode & 0o111),
    } for p in root.rglob('*') if p.is_file() and '.git' not in p.relative_to(root).parts}


def probe(root):
    source = root / 'source'
    source.mkdir()
    git(source, 'init', '-q', '--initial-branch=main')
    git(source, 'config', 'uploadpack.allowFilter', 'true')
    expected_paths = ['scripts/run.sh', 'src/core/model.txt',
                      'src/server/effects/io.js', 'test/tools/driver.js']
    for name in expected_paths:
        p = source / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text('fixture: ' + name + '\n')
    (source / 'scripts/run.sh').chmod(0o755)
    (source / 'README.md').write_text('Root files remain in cone sparse checkout.\n')
    rng = random.Random(20261102)
    obsolete = source / 'obsolete.bin'
    commits = []
    for i in range(6):
        obsolete.write_bytes(rng.randbytes(256 * 1024))
        git(source, 'add', '.')
        git(source, 'commit', '-qm', f'history {i}')
        commits.append(git(source, 'rev-parse', 'HEAD').stdout.strip())
    old_blob = git(source, 'rev-parse', commits[0] + ':obsolete.bin').stdout.strip()
    git(source, 'branch', 'feature', commits[2])
    obsolete.unlink()
    (source / 'assets').mkdir()
    (source / 'assets/current.bin').write_bytes(rng.randbytes(256 * 1024))
    git(source, 'add', '-A')
    git(source, 'commit', '-qm', 'current tree')
    head = git(source, 'rev-parse', 'HEAD').stdout.strip()
    expected = inventory(source)
    sparse_paths = ['scripts', 'src/core', 'src/server/effects', 'test/tools']
    sparse_expected = {k: v for k, v in expected.items() if not k.startswith('assets/')}
    rows = {}
    for mode, shallow, filtered, sparse in [
        ('shallow', True, False, False),
        ('full', False, False, False),
        ('blobless', False, True, False),
        ('sparse_shallow', True, True, True),
        ('sparse_blobless', False, True, True),
    ]:
        dest = root / mode
        command = ['clone', '-q', '--no-checkout', '--branch', 'main']
        if shallow:
            command += ['--depth', '1']
        if filtered:
            command += ['--filter=blob:none']
        command += [source.as_uri(), str(dest)]
        start = time.perf_counter()
        cloned = git(root, *command)
        if 'filtering not recognized' in cloned.stderr:
            raise ValueError('server ignored filter')
        if sparse:
            git(dest, 'sparse-checkout', 'set', '--cone', *sparse_paths)
        git(dest, 'checkout', '-q', '--detach', head)
        seconds = time.perf_counter() - start
        if inventory(dest) != (sparse_expected if sparse else expected):
            raise ValueError(mode + ': materialized bytes/modes differ')
        count = int(git(dest, 'rev-list', '--count', 'HEAD').stdout)
        if count != (1 if shallow else 7):
            raise ValueError(mode + ': wrong history depth')
        base = git(dest, 'merge-base', 'HEAD', 'origin/feature', check=False)
        if shallow:
            if base.returncode == 0:
                raise ValueError('shallow negative control unexpectedly found base')
        elif base.returncode or base.stdout.strip() != commits[2]:
            raise ValueError(mode + ': merge-base unavailable/wrong')
        # rev-list --missing=print enumerates absent objects without fetching them.
        missing = {line[1:] for line in git(
            dest, 'rev-list', '--objects', '--all', '--missing=print'
        ).stdout.splitlines() if line.startswith('?')}
        if filtered and not shallow and old_blob not in missing:
            raise ValueError(mode + ': obsolete historical blob was fetched')
        if not filtered and missing:
            raise ValueError(mode + ': unexpected missing objects')
        rows[mode] = {
            'clone_checkout_seconds': seconds,
            'packed_object_bytes': sum(p.stat().st_size for p in
                                       (dest / '.git/objects/pack').glob('*.pack')),
            'head_commits': count, 'merge_base_available': base.returncode == 0,
            'missing_objects': len(missing), 'materialized_files': len(inventory(dest)),
            'head_bytes_and_modes_exact': True,
            'historical_blob_omitted': old_blob in missing,
        }
    return {'fixture_head': head, 'expected_merge_base': commits[2], 'rows': rows,
            'controls': {'shallow_lacks_merge_base': True,
                         'full_history_blobless_omits_old_blob': True,
                         'sparse_preserves_selected_head_bytes_and_modes': True}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path,
                        default=Path('build/ci-speed/checkout-probe.json'))
    args = parser.parse_args()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.unlink(missing_ok=True)
    start = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix='bp-checkout-') as tmp:
        result = probe(Path(tmp))
    result.update({
        'git': git(Path.cwd(), '--version').stdout.strip(),
        'probe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'total_seconds': time.perf_counter() - start,
        'scope': 'One disposable local file:// transport fixture; no network/Actions speed claim',
    })
    args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
