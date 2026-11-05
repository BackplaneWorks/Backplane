"""Tiny controls prepared for bounded remote producer preflight; no local execution."""
import copy
import unittest
import policy_checks as P


class Controls(unittest.TestCase):
    def setUp(self):
        self.commit, self.tree, self.compiler = '1'*40, '2'*40, '3'*64
        identity = {'sourceCommit': self.commit, 'sourceTree': self.tree}
        self.selected = {**identity, 'excludedPRs': [932, 947],
            'normalMergedCommits': {str(n): '7'*40 for n in (948, 950, 951, 954, 928, 923, 922)},
            'lawRemoval': {'state': 'EXPLICIT_USER_APPROVED',
                'laws': ['cad_install_visible_action', 'cad_install_monotonic'],
                'receipt': 'consent.json', 'sha256': '8'*64},
            'fresh24': {**identity, 'verdict': 'PASS', 'passed': 24, 'total': 24,
                'inherited': False, 'receipt': 'fresh.json', 'sha256': '4'*64},
            'pinManifestReview': {**identity, 'verdict': 'PASS', 'receipt': 'review.json',
                'sha256': '5'*64, 'manifest': 'pins.json', 'manifestSha256': '6'*64}}
        self.phase = {**identity, 'exitCode': 0, 'hardMemoryCapBytes': P.CAP, 'swapMaxBytes': 0,
            'compilerArchiveSHA256': self.compiler, 'seconds': 1.2, 'peakRSSKiB': 1000, 'memoryPeakBytes': 2000000}
        self.end = {'exitCode': 0, 'compilerPostIdentity': True, 'memoryPeakBytes': 2000000,
            'memoryEvents': 'oom 0\noom_kill 0\noom_group_kill 0\n'}

    def test_valid_exact_source_selection(self):
        P.selection(self.selected, self.commit, self.tree)

    def test_947_retained_rejected(self):
        self.selected['excludedPRs'] = [932]
        with self.assertRaises(ValueError): P.selection(self.selected, self.commit, self.tree)

    def test_missing_additional_merge_rejected(self):
        for n in (954, 928, 923, 922):
            value = copy.deepcopy(self.selected); del value['normalMergedCommits'][str(n)]
            with self.subTest(pr=n), self.assertRaises(ValueError): P.selection(value, self.commit, self.tree)

    def test_pending_or_broader_law_consent_rejected(self):
        for key, wrong in (('state', 'PENDING'), ('receipt', None),
                           ('laws', ['cad_install_visible_action', 'cad_install_monotonic', 'unrelated'])):
            value = copy.deepcopy(self.selected); value['lawRemoval'][key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError): P.selection(value, self.commit, self.tree)

    def test_missing_fresh24_rejected(self):
        del self.selected['fresh24']
        with self.assertRaises(ValueError): P.selection(self.selected, self.commit, self.tree)

    def test_wrong_new_source_and_old24_rejected(self):
        for key in ('sourceCommit', 'sourceTree'):
            value = copy.deepcopy(self.selected); value['fresh24'][key] = '0'*40
            with self.subTest(key=key), self.assertRaises(ValueError): P.selection(value, self.commit, self.tree)

    def test_inherited_and_incomplete24_rejected(self):
        for key, wrong in (('inherited', True), ('passed', 23), ('total', 25), ('receipt', None), ('sha256', None)):
            value = copy.deepcopy(self.selected); value['fresh24'][key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError): P.selection(value, self.commit, self.tree)

    def test_missing_or_wrong_source_pin_review_rejected(self):
        for key in ('receipt', 'manifestSha256', 'sourceTree'):
            value = copy.deepcopy(self.selected); value['pinManifestReview'][key] = None
            with self.subTest(key=key), self.assertRaises(ValueError): P.selection(value, self.commit, self.tree)

    def test_old_receipt_source_rejected_even_when_pointer_claims_new(self):
        with self.assertRaises(ValueError): P.source_identity({'commit': '0'*40, 'tree': '0'*40}, self.commit, self.tree)

    def test_valid_capped_consistent_actual_peak(self):
        P.phase(self.phase, self.end, self.commit, self.tree, self.compiler)

    def test_missing_nonpositive_or_overcap_kernel_peak_rejected(self):
        for peak in (None, 0, -1, P.CAP+1):
            value = dict(self.phase, memoryPeakBytes=peak)
            with self.subTest(peak=peak), self.assertRaises(ValueError): P.phase(value, self.end, self.commit, self.tree, self.compiler)

    def test_mismatched_raw_exit_kernel_peak_rejected(self):
        for peak in (None, 0, P.CAP+1, self.end['memoryPeakBytes']+1):
            value = dict(self.end, memoryPeakBytes=peak)
            with self.subTest(peak=peak), self.assertRaises(ValueError): P.phase(self.phase, value, self.commit, self.tree, self.compiler)

    def test_wrong_caps_swap_compiler_source_and_missing_metrics_rejected(self):
        for key, wrong in (('hardMemoryCapBytes', P.CAP*2), ('swapMaxBytes', 1),
                           ('compilerArchiveSHA256', '0'*64), ('sourceTree', '0'*40), ('seconds', 0), ('peakRSSKiB', 0)):
            value = dict(self.phase); value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError): P.phase(value, self.end, self.commit, self.tree, self.compiler)

    def test_failed_compiler_oom_or_postidentity_rejected(self):
        for key, wrong in (('exitCode', 137), ('compilerPostIdentity', False), ('memoryEvents', 'oom 1\noom_kill 1\n')):
            value = dict(self.end); value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError): P.phase(self.phase, value, self.commit, self.tree, self.compiler)


if __name__ == '__main__':
    unittest.main(verbosity=2)
