"""Corruption controls for the saved-trace audit; no compiler invocation."""
import copy
import json
from pathlib import Path
import runpy
import unittest

api = runpy.run_path(str(Path(__file__).with_name('review.py')))
files = api['evidence']()
summary = json.loads(files['artifact/SUMMARY.json'])
proof_file = summary['command_results'][1]['execution']['command'][2]
marks = [json.loads(line) for line in files['artifact/positive/profile.jsonl'].splitlines()]
expected = {'LAWS:'+name for name in summary['primary_laws']} | {'dm_append_nil'}


class Controls(unittest.TestCase):
    def audit(self, changed):
        return api['trace_coverage'](changed, expected, proof_file)

    def test_saved_trace_and_source(self):
        self.assertEqual(self.audit(marks)['missing_checked_keys'], [])
        receipt = api['review']()
        self.assertFalse(receipt['ci_gate_qualified'])
        self.assertEqual(receipt['compiler_memory_max_events_delta'], 1675)

    def test_missing_last_validation(self):
        changed = copy.deepcopy(marks)
        at = max(i for i, m in enumerate(changed) if m['phase'] == 'events:validated')
        del changed[at]
        with self.assertRaises(AssertionError): self.audit(changed)

    def test_wrong_phase_from_original_harness(self):
        changed = copy.deepcopy(marks)
        for m in changed:
            if m['phase'] == 'events:validated': m['phase'] = 'events:before-check'
        with self.assertRaises(AssertionError): self.audit(changed)

    def test_all_three_batches_omit_law(self):
        changed = copy.deepcopy(marks)
        for m in changed:
            if 'names' in m: m['names'] = [n for n in m['names'] if n != 'LAWS:watch_copy_intent_none']
        with self.assertRaises(AssertionError): self.audit(changed)

    def test_source_spelling_is_not_kernel_key(self):
        changed = copy.deepcopy(marks)
        for m in changed:
            if 'names' in m: m['names'] = [n.replace('LAWS:', 'Laws.') for n in m['names']]
        with self.assertRaises(AssertionError): self.audit(changed)

    def test_non_root_events_cannot_cover_root(self):
        with self.assertRaises(AssertionError):
            api['trace_coverage'](marks, expected, proof_file+'.wrong')

    def test_prefix_gap(self):
        changed = copy.deepcopy(marks)
        next(m for m in changed if m['phase'] == 'events:check:start')['from'] += 1
        with self.assertRaises(AssertionError): self.audit(changed)

    def test_truncated_completion(self):
        at = next(i for i, m in enumerate(marks) if m['phase'] == 'modules:complete')
        with self.assertRaises(AssertionError): self.audit(marks[:at])

    def test_fallback(self):
        with self.assertRaises(AssertionError): self.audit(marks + [{'phase':'modules:fallback'}])

    def test_law_and_fill_hashes_are_distinct(self):
        block_hashes = api['block_hashes']
        a = block_hashes('law helper:\n  {0n == 0n : Nat}\n\ndef helper(): {==}\n')
        b = block_hashes('law helper:\n  {0n == 1n : Nat}\n\ndef helper(): {==}\n')
        self.assertNotEqual(a[('law', 'helper')], b[('law', 'helper')])
        self.assertEqual(a[('def', 'helper')], b[('def', 'helper')])


if __name__ == '__main__':
    unittest.main()
