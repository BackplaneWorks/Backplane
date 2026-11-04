"""Offline audit of ONE saved experiment; never executes Bend or records cache passes."""
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile

HERE = Path(__file__).resolve().parent
COMPILER = 3699998720
CONTROLLER = 536870912


def digest(data):
    return hashlib.sha256(data).hexdigest()


def evidence():
    data = (HERE/'EVIDENCE.tar.gz').read_bytes()
    index = json.loads((HERE/'EVIDENCE.json').read_text())
    assert digest(data) == index['bundle_sha256'], 'bundle changed'
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as tar:
        members = tar.getmembers()
        assert len({m.name for m in members}) == len(members)
        assert all(m.isfile() for m in members)
        files = {m.name: tar.extractfile(m).read() for m in members}
    assert {n: digest(v) for n, v in files.items()} == index['files']
    return files


def trace_coverage(marks, expected, proof_file):
    """Require paired successful batches, contiguous event ranges and root completion."""
    active = pending = validated = None
    cursor = 0
    names = set()
    root_groups = []
    root_closed = False
    completed = None
    for mark in marks:
        phase = mark['phase']
        assert phase != 'modules:fallback', 'whole-book fallback'
        if phase == 'module:check:start':
            assert active is None and pending is None
            active = mark['file']
            # Deferred files keep pending; the eventual root validates the
            # entire tail. 368 files need only 361 module check spans here.
            assert mark['done'] == cursor, 'skipped or repeated checked prefix'
        elif phase == 'events:check:start':
            assert active is not None and pending is None
            assert mark['from'] == cursor and mark['to'] > cursor
            assert mark['names'] and len(set(mark['names'])) == len(mark['names'])
            pending = (mark['from'], mark['to'], mark['names'])
        elif phase == 'events:validated':
            assert pending is not None and validated is None
            assert pending == (mark['from'], mark['to'], mark['names'])
            validated = pending
        elif phase == 'events:check:end':
            assert pending is not None and validated == pending
            assert pending == (mark['from'], mark['to'], mark['names'])
            if active == proof_file:
                names.update(mark['names'])
                root_groups.append({'from': mark['from'], 'to': mark['to'], 'names': mark['names']})
            cursor = mark['to']
            pending = validated = None
        elif phase == 'module:check:end':
            assert active == mark['file'] and pending is None
            if active == proof_file:
                root_closed = True
            active = None
        elif phase == 'modules:complete':
            assert active is None and pending is None and completed is None
            assert root_closed and mark['discard_results'] is True and mark['paged'] is True
            completed = mark
        elif phase == 'load:end':
            assert completed is not None and mark['order'] == cursor
    assert active is None and pending is None and completed is not None
    assert expected <= names, 'missing original filled proof events'
    return {'checked_event_count': cursor, 'expected_checked_keys': sorted(expected),
            'missing_checked_keys': sorted(expected - names), 'root_batches': root_groups}


def block_hashes(text):
    # Canonical declaration text includes trailing comments/blank lines. Keep
    # law and fill events separate even when they have the same source name.
    result, current = {}, None
    for line in text.splitlines(keepends=True):
        if line and not line[0].isspace() and not line.startswith('#'):
            if line.startswith('import '):
                current = None
                continue
            match = re.match(r'^(law|def|type) ([\w.]+)', line)
            assert match, 'unknown canonical declaration'
            current = (match[1], match[2])
            assert current not in result
            result[current] = line
        elif current is not None:
            result[current] += line
    return {key: digest(value.encode()) for key, value in result.items()}


def review():
    files = evidence()
    read = lambda name: files[name].decode()
    obj = lambda name: json.loads(read(name))
    summary = obj('artifact/SUMMARY.json')
    identity = obj('frozen/IDENTITY.json')
    assert digest(files['frozen/IDENTITY.json']) == summary['identity_sha256']
    packet = obj('artifact/packet-result.json')
    assert packet['inputs_unchanged'] is True and packet['returncode'] == 1
    assert packet['packet_sha256'] == '5572324b21fcda62051d5f64b2cc285391a2cb1e7fa7c358c1465d8e17723c07'
    for name in identity['compiler_files']:
        assert digest(files['frozen/compiler/'+name]) == identity['compiler_files'][name]
    for name in ('run.py', 'inner.py', 'prepare.py', 'CHECKER.patch', 'negative.bend',
                 'source/build/proof-shards/plan_cur/manifest.json',
                 'source/build/proof-shards/plan_cur/000/PROOF.bend',
                 'source/build/proof-shards/plan_cur/000/LAWS.bend'):
        assert digest(files['frozen/'+name]) == identity['files'][name]['sha256']
    assert summary['state'] == 'failed', 'preserve original harness outcome'
    assert summary['combined_hard_cap_bytes'] == COMPILER + CONTROLLER < 4*1024**3
    assert summary['controller_memory_max_bytes'] == CONTROLLER and summary['controller_swap_max'] == 0
    assert summary['controller_peak_bytes'] <= CONTROLLER
    for event in ('oom', 'oom_kill'):
        assert summary['controller_events_after'][event] == 0
    negative, positive = summary['command_results']
    for row in (negative, positive):
        assert not row['timed_out'] and 'oom-kill' not in row['unit_result']
        execution = row['execution']
        assert execution['actual_bend_flock_held'] is True
        assert execution['checker_exit'] == row['unit_exit'] == 1
        for state in ('before', 'after'):
            cap = execution[state]
            assert cap['memory_max_bytes'] == COMPILER and cap['swap_max'] == 0
            assert cap['memory_peak_bytes'] <= COMPILER
            assert cap['events']['oom'] == cap['events']['oom_kill'] == 0
    assert negative['false_theorem_rejected'] is True and negative['inputs_tools_unchanged'] is True
    negative_log = read('artifact/negative/checker.log')
    assert 'Location: bad' in negative_log and '- expected :' in negative_log
    # Exact recorded positive report: ONLY the existing unsafe/foreign notice.
    log = read('artifact/positive/checker.log').splitlines()
    assert log[:2] == ['SOME PROOFS FAIL', 'Error: 2574 defs rely on unsafe or foreign code:']
    assert len(log[2:]) == 2574 and all(re.fullmatch(r'- [^ ]+', line) for line in log[2:])
    assert digest(files['artifact/positive/checker.log']) == positive['log_sha256']
    proof = read('frozen/source/build/proof-shards/plan_cur/000/PROOF.bend')
    for origin in ('LAWS.bend', 'PROOF.bend'):
        original_blocks = block_hashes(read('frozen/source/'+origin))
        emitted_blocks = block_hashes(read('frozen/source/build/proof-shards/plan_cur/000/'+origin))
        for key, sha in emitted_blocks.items():
            assert original_blocks[key] == sha
        # The original preparation receipt keys by name, so the helper's
        # later def replaces its law hash. Above we compare BOTH events.
        assert {key[1]: sha for key, sha in emitted_blocks.items()} == identity['original_block_hashes'][origin]
    assert re.findall(r'^import (.+) as Laws$', proof, re.M) == ['./LAWS.bend']
    declared = re.findall(r'^def ([\w.]+)\(', proof, re.M)
    original = identity['original_block_hashes']['PROOF.bend']
    assert len(declared) == len(set(declared)) and set(declared) == set(original)
    assert set(declared) == {'Laws.'+n for n in identity['primary_laws']} | {'dm_append_nil'}
    # Frozen bend.ts book_load derives LAWS namespace from ./LAWS.bend;
    # parse_reso resolves alias Laws.x to LAWS:x, parse_def fills that law.
    # check_paged expands a group through each key's LAST event; book_valid
    # def_check checks its filled body at that event before events:validated.
    expected = {n.replace('Laws.', 'LAWS:', 1) if n.startswith('Laws.') else n for n in declared}
    proof_file = positive['execution']['command'][2]
    marks = [json.loads(line) for line in read('artifact/positive/profile.jsonl').splitlines()]
    coverage = trace_coverage(marks, expected, proof_file)
    positive_events = positive['execution']['after']['events']
    assert positive_events['max'] >= positive['execution']['before']['events']['max']
    peak = int(re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', read('artifact/positive/checker.time'))[1])
    assert peak == positive['peak_rss_kib']
    return {'schema': 1, 'classification': 'saved checker completed; original trace verifier defective',
            'original_harness_exit': 1, 'original_harness_state': summary['state'],
            'positive_raw_checker_exit': 1, 'existing_verdict_policy_accepted': True,
            'unsafe_or_foreign_defs': 2574, 'positive_compiler_seconds': positive['execution']['seconds'],
            'positive_compiler_peak_rss_kib': peak,
            'compiler_peak_cgroup_bytes': positive['execution']['after']['memory_peak_bytes'],
            'compiler_memory_events_after': positive_events,
            'compiler_memory_max_events_delta': positive_events['max'] - positive['execution']['before']['events']['max'],
            'controller_peak_cgroup_bytes': summary['controller_peak_bytes'],
            'combined_hard_cap_bytes': COMPILER + CONTROLLER,
            'false_theorem_rejected': True, 'primary_laws': 25,
            'negative_scope': 'error surfacing on a small book; not a corrupted full shard',
            'definition_count': 34067, 'event_count': 33925,
            'definition_minus_event_count': 142,
            'definition_minus_event_classification': 'unclassified; do not infer all definitions have events',
            'stock_checker_equivalence_established': False,
            'full_proof_qualified': False, 'ci_gate_qualified': False, 'release_qualified': False,
            'job': 'j20261103235057-263893', 'work': 'w-t1793650867-1261555-1465396',
            'paid_credit_seconds': 703, 'paid_cost_dollars': 0.00703,
            'evidence_bundle_sha256': digest((HERE/'EVIDENCE.tar.gz').read_bytes()),
            'coverage': coverage}


if __name__ == '__main__':
    print(json.dumps(review(), indent=2, sort_keys=True))
