"""Recompute scoped observations from saved receipts. Never runs a compiler."""
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile

HERE = Path(__file__).resolve().parent


def summarize():
    sha = lambda data: hashlib.sha256(data).hexdigest()
    data = (HERE/'EVIDENCE.tar.gz').read_bytes()
    index = json.loads((HERE/'EVIDENCE.json').read_text())
    assert sha(data) == index['bundle_sha256']
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as tar:
        members = tar.getmembers()
        assert len({m.name for m in members}) == len(members)
        assert all(m.isfile() for m in members)
        files = {m.name: tar.extractfile(m).read() for m in members}
    assert {n: sha(value) for n, value in files.items()} == index['files']
    obj = lambda name: json.loads(files[name])
    records = {}
    for label, suffix in [('js', '.js'), ('mjs', '.mjs'), ('mjs-context', '.mjs')]:
        r = obj(label+'/RESULT.json')
        packet = obj(label+'/packet-result.json')
        assert r['state'] == 'passed' and r['raw_exit'] == 0 and not r['timed_out']
        assert r['inputs_unchanged'] and packet['inputs_unchanged'] and packet['returncode'] == 0
        assert r['byte_identical'] and r['focused_output_sha256'] == r['reference_output_sha256']
        assert sha(files[label+'/focused'+suffix]) == r['focused_output_sha256']
        peak = int(re.search(rb'Maximum resident set size \(kbytes\):\s*(\d+)', files[label+'/compiler.time'])[1])
        assert peak == r['peak_rss_kib']
        compiler_cap = r['compiler_scope_cap']['memory_max_bytes']
        assert compiler_cap == 3699998720 and r['memory_max_bytes'] == 536870912
        assert r['compiler_scope_cap']['swap_max'] == r['swap_max'] == 0
        actual_cap_sum = compiler_cap + r['memory_max_bytes']
        assert actual_cap_sum < 4*1024**3
        samples = [json.loads(line) for line in files[label+'/memory-stat.jsonl'].splitlines()]
        assert samples
        max_events = {k: max(m['events'].get(k, 0) for m in samples)
                      for k in ('max', 'oom', 'oom_kill')}
        assert max_events['oom'] == max_events['oom_kill'] == 0
        sampled_peak = max(m['peak'] for m in samples)
        assert sampled_peak <= compiler_cap
        records[label] = {
            'compiler_files': r['compiler_files'], 'compiler_scope_seconds': r['seconds'],
            'compiler_peak_rss_kib': peak, 'sampled_cgroup_peak_bytes': sampled_peak,
            'total_cap_minus_sampled_peak_bytes': compiler_cap - sampled_peak,
            'max_sampled_memory_events': max_events,
            'controller_peak_bytes': r['controller_peak_bytes'],
            'actual_combined_hard_cap_bytes': actual_cap_sum,
            'raw_reported_combined_hard_cap_bytes': r['combined_hard_cap_bytes'],
            'stored_bytes': r['module_completions'][0]['stored_bytes'],
            'output_sha256': r['focused_output_sha256'],
            'packet_sha256': packet['packet_sha256'], 'packet_source_files': packet['source_files'],
            'inputs_unchanged': True,
        }
        if label in ('js', 'mjs'):
            text = files[label+'/work.out'].decode()
            cleanup = re.search(r'sandbox (\S+) deleted \(verified\); used (\d+) credit-s \(\$([^)]*)\)', text)
            assert cleanup, 'missing saved cleanup/cost evidence'
            records[label].update(sandbox=cleanup[1], sandbox_deleted_verified=True,
                                  paid_credit_seconds=int(cleanup[2]), paid_cost_dollars=float(cleanup[3]))
    # Both observations used patched compilers and DIFFERENT compositions.
    a, b = records['mjs-context']['compiler_files'], records['mjs']['compiler_files']
    changed = {k: {'context': a[k], 'graph2': b[k]} for k in a if a[k] != b[k]}
    assert set(changed) == {'comp.ts', 'offload.ts'}
    slow = [json.loads(line) for line in files['mjs/l5slow.jsonl'].splitlines()]
    largest = max(m['blob_bytes'] for m in slow)
    decodes = [m for m in slow if m['blob_bytes'] == largest]
    assert len(decodes) == 2 and [m['times_read'] for m in decodes] == [1, 2]
    assert len({m['id'] for m in decodes}) == 1
    controls = obj('lane5/graph2-controls.txt')
    assertions = sum(v.startswith('ok') for v in controls['results'].values())
    info = sum(v.startswith('info') for v in controls['results'].values())
    assert controls['failures'] == 0 and assertions == 26 and info == 1
    assert sha(files['lane5/offload.ts']) == records['js']['compiler_files']['offload.ts']
    failed = {}
    for label in ('failed-371652', 'failed-379465'):
        text = files[label+'/work.out'].decode()
        cleanup = re.search(r'sandbox (\S+) deleted \(verified\); used (\d+) credit-s \(\$([^)]*)\)', text)
        assert cleanup
        failed[label] = {'paid_credit_seconds': int(cleanup[2]),
                         'paid_cost_dollars': float(cleanup[3]),
                         'sandbox_deleted_verified': True,
                         'scope': 'failed preflight; no compiler measurement'}
    return {
        'kind': 'saved graph2 focused observations; distinct compositions, n=1 each',
        'full_build_qualified': False, 'full_proof_qualified': False,
        'ci_gate_qualified': False, 'release_qualified': False,
        'new_compiler_or_paid_runs': 0,
        'observations': records, 'mjs_composition_changes_against_context': changed,
        'largest_blob_read_log': decodes,
        'largest_blob_two_decode_seconds': sum(m['decode_ms'] for m in decodes)/1000,
        'graph_format_assertions': assertions, 'graph_format_info_rows': info,
        'graph_patch_sha256': sha(files['lane5/graph2.patch']),
        'failed_preflight_costs': failed,
        'successful_graph2_credit_seconds': sum(records[k]['paid_credit_seconds'] for k in ('js', 'mjs')),
        'total_graph2_credit_seconds_including_failed_preflights':
            sum(records[k]['paid_credit_seconds'] for k in ('js', 'mjs')) + sum(r['paid_credit_seconds'] for r in failed.values()),
        'independent_patch_review': 'review/PATCH-REVIEW.json inside evidence bundle',
        'limits': [
            'No matched causal before/after compiler benchmark; comp.ts and offload.ts both changed in mjs context.',
            'D-only JS comparison also includes interface sharing; whole spill reduction is not attributed solely to format.',
            'Anonymous-memory gap excludes file cache and kernel; total cgroup margin is reported separately.',
            'Memory events are sampled; zero sampled events is scoped evidence, not an independent host attestation.',
            'Raw inherited comparison/cap fields remain unchanged; additive values above are recomputed.',
            'No full hub/web, full proof, package, install, release dispatch or cache pass recording.',
        ],
    }


if __name__ == '__main__':
    print(json.dumps(summarize(), sort_keys=True, indent=2))
