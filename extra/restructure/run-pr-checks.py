"""Light PR checks on Boat only, including a frozen merged-law snapshot.

No compiler, full proof gate, cache promotion, build or installation. The entire
test process tree runs in the Boat command's verified 512 MiB/no-swap cgroup.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import re
import shutil
import subprocess
import sys
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_audit(root, identity):
    """Optional mechanical declaration/law equivalence checks, on Boat only."""
    requested = identity.get('source_audit')
    if requested is None:
        return None
    spec = importlib.util.spec_from_file_location('source_reader', root/'scripts/proof-slice.py')
    reader = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reader)

    def normalized(text, replacements):
        result, offset = [], 0
        for token in reader.LEX.finditer(text):
            if token[1] in replacements:
                result.extend([text[offset:token.start(1)], replacements[token[1]]])
                offset = token.end(1)
        return ''.join(result)+text[offset:]

    def declarations(name, replacements=None):
        text = (root/name).read_text()
        return reader.scan_module(normalized(text, replacements or {}), name)[1]

    equivalent = []
    partitions = []
    forwarders = []
    for entry in requested.get('forwarders', []):
        header = next(line for line in (root/entry['before']).read_text().splitlines()
                      if line.startswith('def '+entry['name']+'('))
        expected = reader.scan_module('import Base\n'+header+'\n  '+entry['call']+'\n', 'forwarder')[1]
        actual = declarations(entry['after'])
        key = ('def', entry['name'])
        assert actual[key] == expected[key], (entry['after'], key, 'nontransparent delegate')
        alias = entry['call'].split('.', 1)[0]
        imports = dict((a, p) for p, a in re.findall(r'^import (\S+) as (\w+)',
                                                    (root/entry['after']).read_text(), re.M))
        assert ((root/entry['after']).parent/imports[alias]).resolve() == (root/entry['target_module']).resolve()
        forwarders.append([entry['after'], entry['name'], entry['target_module']])
    for entry in requested.get('partitions', []):
        before = declarations(entry['before'])
        combined = {}
        for part in entry['after']:
            after = declarations(part['path'], part.get('replacements'))
            assert not set(combined) & set(after), 'duplicate relocated declaration'
            combined.update(after)
        assert combined == before, 'relocation changed, added or lost a declaration'
        partitions.append({'declarations': len(before),
                           'files': [part['path'] for part in entry['after']],
                           'all_bodies_preserved': True})
    for entry in requested['declarations']:
        before = declarations(entry['before'], entry.get('before_replacements'))
        after = declarations(entry['after'], entry.get('after_replacements'))
        for kind, name in entry['names']:
            assert before[(kind, name)] == after[(kind, name)], (entry['after'], kind, name)
            equivalent.append([entry['after'], kind, name, after[(kind, name)]])
    for entry in requested.get('remaining', []):
        before = declarations(entry['before'])
        after = declarations(entry['after'], entry.get('after_replacements'))
        removed = {tuple(k) for k in entry['removed']}
        assert set(after) == set(before)-removed
        for key, digest in after.items():
            assert before[key] == digest, (entry['after'], key)
    for entry in requested.get('books', []):
        before = declarations(entry['before'])
        after = declarations(entry['after'], entry.get('after_replacements'))
        changed = {tuple(k) for k in entry['changed']}
        assert set(after)-set(before) == {tuple(k) for k in entry['added']}
        assert set(before) <= set(after)
        for key in set(before)-changed:
            assert before[key] == after[key], (entry['after'], key)
    laws = requested['laws']
    before = declarations(laws['before'])
    after = declarations(laws['after'], laws['after_replacements'])
    for key, digest in before.items():
        assert after[key] == digest, key
    assert set(after)-set(before) == {('law', n) for n in laws['added']}
    result = {'equivalent_declarations': equivalent, 'remaining_declarations_unchanged': True,
              'all_original_law_statements_preserved': True, 'added_laws': laws['added']}
    result['partitions'] = partitions
    result['transparent_forwarders'] = forwarders
    if requested.get('phone_state'):
        phone = requested['phone_state']
        assert sha(root/'scripts/state-key.py') == phone['script_sha256']
        assert sha(root/'.github/actions/setup-bend/action.yml') == phone['compiler_pin_sha256']
        covered = subprocess.check_output([sys.executable, 'scripts/state-key.py', '--types'], text=True)
        paths = {line.split()[0] for line in covered.splitlines()}
        typed_changes = sorted(paths & set(phone['changed_files']))
        for name in typed_changes:
            original = declarations(identity['base_files'][name])
            current = declarations(name)
            assert {k: v for k, v in original.items() if k[0] == 'type'} == {
                k: v for k, v in current.items() if k[0] == 'type'}, 'changed phone-kept type declaration'
        result['phone_state'] = {
            'key': subprocess.check_output([sys.executable, 'scripts/state-key.py'], text=True).strip(),
            'covered_files': sorted(paths), 'changed_inputs_in_covered_graph': [],
            'changed_files_with_identical_type_blocks': typed_changes,
            'script_and_compiler_pin_unchanged': True}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--identity', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    root = Path.cwd()
    identity_path = args.identity.resolve()
    identity_path.relative_to(root)
    out = args.out.resolve()
    assert out.relative_to(root).parts[0] == 'build' and not out.exists()
    assert os.environ.get('BACKPLANE_HEAVY_BOAT_SANDBOX') == os.environ.get('BACKPLANE_HEAVY_OFFLOADED') == '1'
    group = Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
    assert group.name.startswith('ascii-cmd-')
    subprocess.run(['systemctl', '--user', '--no-ask-password', 'set-property', '--runtime',
                    group.name, 'MemoryMax=536870912', 'MemorySwapMax=0'], check=True)
    assert (group/'memory.max').read_text().strip() == '536870912'
    assert (group/'memory.swap.max').read_text().strip() == '0'
    out.mkdir(parents=True)
    identity = json.loads(identity_path.read_text())
    for name, digest in identity['files'].items():
        assert sha(root/name) == digest, name
    audit = source_audit(root, identity)
    for name, snapshot in identity['base_files'].items():
        target = out/'base'/name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root/snapshot, target)
    python = str(Path(sys.executable).resolve())
    commands = [[python, '-B', '-m', 'unittest', 'discover', '-s', 'test/tools', '-p', pattern]
                for pattern in ('dependency_map_test.py', 'proof_shards_test.py', 'proof_slice_test.py')]
    commands.extend([
        [python, '-B', 'scripts/laws-shape.py'],
        [python, '-B', 'scripts/laws-shape.py', identity['merge_laws'], identity['merge_proof']],
        ['git', 'apply', '--check', '--whitespace=error-all',
         '--directory='+str(out.relative_to(root)/'base'), identity['code_patch']],
    ])
    if identity.get('i18n_checks'):
        commands.extend([
            [python, '-B', 'scripts/i18n-gen.py', '--self-test'],
            [python, '-B', 'scripts/i18n-gen.py', '--check'],
            [python, '-B', '-m', 'unittest', 'discover', '-s', 'test/tools',
             '-p', 'i18n_native_codegen_test.py'],
        ])
    observations = []
    environment = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
    failed = False
    for index, command in enumerate(commands):
        started = time.monotonic()
        done = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True, timeout=120)
        observation = {'argv': command, 'exit': done.returncode,
                       'seconds': time.monotonic()-started, 'output': done.stdout+done.stderr}
        observations.append(observation)
        (out/('check-'+str(index)+'.txt')).write_text(observation['output'])
        failed |= done.returncode != 0
    for name, digest in identity['files'].items():
        assert sha(root/name) == digest, name
    events = {k: int(v) for k, v in (line.split() for line in (group/'memory.events').read_text().splitlines())}
    assert events['oom'] == events['oom_kill'] == 0
    assert (group/'memory.max').read_text().strip() == '536870912'
    assert (group/'memory.swap.max').read_text().strip() == '0'
    report = {'scope': '35 Python tests, optional i18n controls, canonical and merged-law shape, source patch whitespace/applicability; no compiler/full proof/build',
              'state': 'failed' if failed else 'passed', 'identity_sha256': sha(identity_path),
              'merge_snapshot': identity['merge_snapshot'], 'cgroup': str(group),
              'memory_max_bytes': 536870912, 'memory_swap_max_bytes': 0,
              'memory_peak_bytes': int((group/'memory.peak').read_text()), 'memory_events': events,
              'peak_child_rss_kib': resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
              'inputs_unchanged': True, 'observations': observations, 'full_gate_qualified': False}
    report['source_audit'] = audit
    (out/'RESULT.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'observations'}))
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
