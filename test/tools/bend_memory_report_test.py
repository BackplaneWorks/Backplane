"""Check profile receipts and honest scope without compiling Bend."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('memory_report', ROOT / 'scripts/bend-memory-report.py')
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


class MemoryReportTest(unittest.TestCase):
    def setUp(self):
        names = ['load:start', 'load:end', 'check:start', 'check:end',
                 'analysis:start', 'analysis:end', 'functions:end', 'join:start', 'join:end']
        self.rows = [dict(phase=n, seconds=i, rss_bytes=1000 * i,
                          max_rss_kib=100 * i) for i, n in enumerate(names)]
        self.time = 'Maximum resident set size (kbytes): 900\nExit status: 0\n'

    def test_keeps_checkpoint_and_total_peak_distinct(self):
        value = report.summarize(self.rows, self.time)
        self.assertEqual(value['process_peak_rss_kib'], 900)
        self.assertEqual(value['stages'][0]['end_rss_bytes'], 1000)
        self.assertEqual(value['stages'][0]['cumulative_peak_rss_kib'], 100)
        self.assertIn('not isolated', value['scope'])

    def test_truncated_and_duplicate_profiles_cannot_pass(self):
        for rows in [[], self.rows[:-1], self.rows + [self.rows[-1]]]:
            with self.subTest(rows=len(rows)), self.assertRaises(ValueError):
                report.summarize(rows, self.time)

    def test_invalid_or_nonmonotonic_measurements_rejected(self):
        for field, value in [('seconds', -1), ('rss_bytes', float('nan')),
                             ('max_rss_kib', True), ('seconds', 0)]:
            rows = [dict(r) for r in self.rows]
            rows[-1][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                report.summarize(rows, self.time)

    def test_failed_missing_or_contradictory_process_receipt_rejected(self):
        for timed in ['', self.time.replace('status: 0', 'status: 1'),
                      self.time.replace('900', '700')]:
            with self.subTest(timed=timed), self.assertRaises(ValueError):
                report.summarize(self.rows, timed)


if __name__ == '__main__':
    unittest.main()
